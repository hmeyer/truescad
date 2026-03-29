# TrueScad WASM Webapp Plan

## Phase 1–6: WASM webapp (COMPLETE)

All six phases of the GTK→WASM migration are done. The app builds, runs, and is
deployed to GitHub Pages. The current architecture:

- Lua script → `piccolo` evaluates → `implicit3d` object tree
- `render(w,h)` → CPU ray-marching in Rust → `Uint8ClampedArray` → `ctx.putImageData`
- `tessellate()` → dual contouring → binary STL → Three.js mesh view + export
- `rotate/pan` → update transform matrix in Rust → re-render

---

## Phase 7: GPU Shader Rendering (NEXT)

Replace the CPU ray-marcher with a WebGL2 fragment shader that evaluates the SDF
on the GPU — the same approach used by [sdfer](https://github.com/hmeyer/sdfer).
Drop `implicit3d` as the primitive system and replace it with a minimal
home-grown one that generates both GLSL (for GPU rendering) and supports CPU
evaluation (for tessellation).

### Why

| | Current (CPU) | After (GPU) |
|---|---|---|
| Rendering | Single-threaded WASM, slow | Massively parallel, real-time |
| Interaction | Re-render on each drag event | `requestAnimationFrame` loop, instant |
| Dependencies | `implicit3d` (heavy, separate crate) | Tiny in-tree primitive module |
| Code complexity | Two separate eval paths (Rust renderer + JS canvas) | One GLSL expression, one render loop |

### What stays the same

- Lua scripting API (all Lua function names unchanged)
- `piccolo` scripting engine
- `tessellate()` WASM function and the Three.js mesh view
- `tessellation` crate (still used for dual contouring)
- `nalgebra` (still used for matrix math in transforms)
- CodeMirror 6 editor
- Build pipeline, CI, deploy

---

### New source layout

```
src/
  lib.rs          ← remove render(), add get_shader_source()/get_world_transform()
  luascad.rs      ← swap Box<dyn Object<Float>> → Box<dyn Primitive> throughout
  render.rs       ← DELETE
  primitive.rs    ← new: Primitive trait, Bbox, GlslCtx, clone_box machinery
  primitives/
    sphere.rs
    planes.rs     ← PlaneX/Y/Z/NegX/NegY/NegZ + NormalPlane
    cylinder.rs
    cone.rs
    csg.rs        ← Union, Intersection, Difference (with smooth variants)
    transforms.rs ← Translate, Rotate, Scale (AffineTransformer replacement)
    deform.rs     ← Bender, Twister
  shader.rs       ← new: GLSL template assembly, build_fragment_shader()
web/
  main.js         ← replace putImageData path with WebGL2 setup + rAF loop
```

---

### Step 1 — `src/primitive.rs`: the core trait

```rust
pub trait Primitive: Send + Sync {
    /// Generate GLSL statements into `ctx`; return the name of the float
    /// variable holding the signed distance result.
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String;

    /// CPU evaluation for tessellation. Positive = outside, negative = inside.
    fn eval(&self, p: [f32; 3]) -> f32;

    fn bbox(&self) -> Bbox;
    fn clone_box(&self) -> Box<dyn Primitive>;
}

impl Clone for Box<dyn Primitive> {
    fn clone(&self) -> Self { self.clone_box() }
}

#[derive(Clone, Copy)]
pub struct Bbox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bbox {
    pub fn width(&self) -> f32 {
        (self.max[0] - self.min[0])
            .max(self.max[1] - self.min[1])
            .max(self.max[2] - self.min[2])
    }
}
```

#### `GlslCtx`

```rust
pub struct GlslCtx {
    counter: usize,
    pub statements: Vec<String>,
    pub helpers: IndexSet<String>,  // or Vec + dedup-by-name
}

impl GlslCtx {
    pub fn new() -> Self { ... }
    pub fn fresh_float(&mut self) -> String { let n = self.counter; self.counter += 1; format!("d{n}") }
    pub fn fresh_point(&mut self) -> String { let n = self.counter; self.counter += 1; format!("p{n}") }
    pub fn push(&mut self, s: impl Into<String>) { self.statements.push(s.into()); }
    pub fn add_helper(&mut self, src: &str) { /* insert if not already present */ }
}
```

Use `indexmap::IndexSet` or just `Vec<String>` with a `.contains()` check — either
is fine at this scale. Add `indexmap` to `Cargo.toml` if convenient, or just use
`Vec` with linear scan (the number of distinct helpers is tiny).

---

### Step 2 — `src/primitives/`: one file per primitive family

#### `sphere.rs`

```rust
pub struct Sphere { pub radius: f32 }

impl Primitive for Sphere {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let d = ctx.fresh_float();
        ctx.push(format!("float {d} = length({p}) - {:.8};", self.radius));
        d
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        (x*x + y*y + z*z).sqrt() - self.radius
    }
    fn bbox(&self) -> Bbox {
        let r = self.radius;
        Bbox { min: [-r,-r,-r], max: [r,r,r] }
    }
    fn clone_box(&self) -> Box<dyn Primitive> { Box::new(self.clone()) }
}
```

#### `planes.rs` — 6 axis-aligned planes + NormalPlane

Axis planes are all `dot(p, axis) - d`. E.g.:

```rust
// PlaneX: SDF = p.x - d  (positive outside, i.e. x > d)
// PlaneNegX: SDF = -p.x - d
```

For half-space intersections used by Box and Cylinder (see luascad.rs), these are
composed via `Intersection` in the CSG layer exactly as before.

NormalPlane:
```glsl
float {d} = dot({p}, vec3({nx},{ny},{nz})) - {offset};
```
CPU: `nx*x + ny*y + nz*z - offset`

#### `cylinder.rs` — infinite cylinder along Z

```glsl
float {d} = length({p}.xy) - {radius};
```
CPU: `(x*x + y*y).sqrt() - radius`

Bbox: `[-r,-r,-INF]` to `[r,r,INF]` (capped by intersection with planes in Lua,
same as current `__Cylinder` implementation).

#### `cone.rs` — infinite cone along Z

```glsl
float {d} = dot({p}.xz, normalize(vec2({slope}, 1.0))) + {offset};
```
CPU equivalent using `f32::hypot`.

#### `csg.rs` — Union, Intersection, Difference

Each stores `Vec<Box<dyn Primitive>>` and a `smoothing: f32`.

**Union (`smoothing == 0`):**
```glsl
// evaluate all children → d0, d1, d2...
float {d} = min(d0, min(d1, d2));
```

**Union (`smoothing > 0`):**
Add `smin` helper once:
```glsl
float smin(float a, float b, float k) {
    float h = clamp(0.5 + 0.5*(b-a)/k, 0.0, 1.0);
    return mix(b, a, h) - k*h*(1.0-h);
}
```
Then fold: `smin(d0, smin(d1, d2, k), k)`.

**Intersection:** same with `max` / `smax`.

**Difference:** `max(d0, max(-d1, -d2, ...))` — negate all children after the first.

CPU `eval`: mirror the GLSL logic exactly using `f32::min`, `f32::max`, inline smin.

Bbox for CSG:
- Union: union of all child bboxes
- Intersection: intersection of all child bboxes
- Difference: bbox of the first child (conservative)

#### `transforms.rs` — Translate, Rotate, Scale

Each wraps a `Box<dyn Primitive>` and stores the transform parameters.

**Translate `(tx, ty, tz)`:**
```glsl
vec3 {p1} = {p} - vec3({tx}, {ty}, {tz});
float {d} = /* inner.expression(p1, ctx) */;
```
CPU: subtract translation, forward to inner.

**Rotate (Euler angles `rx, ry, rz`):**
Store the 3×3 rotation matrix at construction time (compute via `nalgebra`).
```glsl
vec3 {p1} = mat3({m00},{m10},{m20}, {m01},{m11},{m21}, {m02},{m12},{m22}) * {p};
```
CPU: multiply the 3×3 matrix by `[x,y,z]`.

**Scale `(sx, sy, sz)` — uniform only for SDF correctness:**
Non-uniform scaling breaks the SDF metric. For now, take a single scale factor `s`:
```glsl
vec3 {p1} = {p} / {s};
float {d_inner} = /* inner */;
float {d} = {d_inner} * {s};
```
If the current Lua API passes a `Vector3` scale, check how luascad.rs calls it —
if users only ever use uniform scale in practice, clamp to `sx`. If non-uniform is
needed, emit the division anyway and accept a slightly incorrect SDF (it will still
render visually OK for mild non-uniform scales, just with incorrect shadow softness).

#### `deform.rs` — Bender, Twister

**Bender `(width)`:**
```glsl
float _by = {p}.y / {width};
vec3 {p1} = vec3(
    {p}.x * cos(_by) - {p}.z * sin(_by),
    {p}.x * sin(_by) + {p}.z * cos(_by),
    {p}.y
);
```

**Twister `(height)`:**
```glsl
float _angle = {p}.z / {height} * 6.28318530718;
float _c = cos(_angle), _s = sin(_angle);
vec3 {p1} = vec3(_c*{p}.x - _s*{p}.y, _s*{p}.x + _c*{p}.y, {p}.z);
```

CPU versions: use `f32::sin_cos`.

---

### Step 3 — `src/shader.rs`: GLSL template

```rust
const TEMPLATE: &str = include_str!("renderer.glsl");

pub fn build_fragment_shader(obj: &dyn Primitive) -> String {
    let mut ctx = GlslCtx::new();
    let result = obj.expression("p", &mut ctx);
    let helpers = ctx.helpers.join("\n");
    let stmts = ctx.statements
        .iter()
        .map(|s| format!("    {s}\n"))
        .collect::<String>();
    let map_fn = format!(
        "float map(vec3 p) {{\n    p = (iWorldTransform * vec4(p, 1.0)).xyz;\n{stmts}    return {result};\n}}"
    );
    format!("{helpers}\n{map_fn}\n{TEMPLATE}")
}
```

`src/renderer.glsl` — the ray-marching template (modelled on sdfer's):

```glsl
precision highp float;
uniform vec2  iResolution;
uniform mat4  iWorldTransform;   // rotated/panned world-to-object matrix

vec3 calcNormal(vec3 p) {
    float e = 0.0001;
    return normalize(vec3(
        map(p + vec3(e,0,0)) - map(p - vec3(e,0,0)),
        map(p + vec3(0,e,0)) - map(p - vec3(0,e,0)),
        map(p + vec3(0,0,e)) - map(p - vec3(0,0,e))
    ));
}

float softShadow(vec3 ro, vec3 rd, float mint, float maxt, float k) {
    float res = 1.0;
    float t = mint;
    for (int i = 0; i < 50; i++) {
        float h = map(ro + rd * t);
        if (h < 0.001) return 0.0;
        res = min(res, k * h / t);
        t += clamp(h, 0.01, 0.2);
        if (t > maxt) break;
    }
    return clamp(res, 0.0, 1.0);
}

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * iResolution) / min(iResolution.x, iResolution.y);

    // Camera
    vec3 ro = vec3(0.0, 0.0, 5.0);
    vec3 rd = normalize(vec3(uv, -1.5));

    // Ray march
    float t = 0.0;
    float tmax = 20.0;
    bool hit = false;
    for (int i = 0; i < 128; i++) {
        float h = map(ro + rd * t);
        if (h < 0.0002) { hit = true; break; }
        if (t > tmax)   { break; }
        t += h;
    }

    vec3 col = vec3(0.12);   // background
    if (hit) {
        vec3 p  = ro + rd * t;
        vec3 n  = calcNormal(p);
        vec3 ld = normalize(vec3(1.0, 2.0, 1.5));

        float diff = clamp(dot(n, ld), 0.0, 1.0);
        float amb  = 0.5 + 0.5 * n.y;
        float sha  = softShadow(p, ld, 0.01, 10.0, 16.0);

        col = vec3(0.7) * (diff * sha + 0.3 * amb);
        col = pow(col, vec3(0.4545));   // gamma
    }

    gl_FragColor = vec4(col, 1.0);
}
```

Tune camera distance / field of view based on `obj.bbox().width()` — pass it as a
uniform `iObjectWidth` and derive `ro.z = iObjectWidth * 2.0` in GLSL, or compute
the camera position in Rust and pass it as a uniform.

---

### Step 4 — `src/lib.rs`: update WASM API

```rust
// REMOVE:
pub fn render(width: u32, height: u32) -> Uint8ClampedArray

// ADD:
/// Returns the GLSL fragment shader source for the current scene,
/// or null if no object is loaded.
#[wasm_bindgen]
pub fn get_shader_source() -> Option<String>

/// Returns the current world transform as a flat 16-element f32 array
/// (column-major, ready for gl.uniformMatrix4fv).
#[wasm_bindgen]
pub fn get_world_transform() -> Vec<f32>

/// Returns the object width (used by JS to set camera distance).
#[wasm_bindgen]
pub fn get_object_width() -> f32
```

`rotate` and `pan` keep the same signatures — they update the internal `nalgebra`
matrix. `get_world_transform()` serialises it.

`AppState` changes:

```rust
struct AppState {
    // renderer: render::Renderer,  ← DELETE
    object: Option<Box<dyn Primitive>>,
    world_transform: na::Matrix4<f32>,
    object_width: f32,
}
```

---

### Step 5 — `src/luascad.rs`: swap primitive type

- Replace all `use implicit3d::*` imports with `use crate::primitive::*` and
  `use crate::primitives::*`
- Change `LObject(Option<Box<dyn Object<Float>>>)` → `LObject(Option<Box<dyn Primitive>>)`
- Each factory function constructs the new primitive type instead of the implicit3d one
- `Box()` Lua function: currently builds 6 planes + Intersection — replace with
  direct `Box3` primitive (simpler GLSL: `length(max(abs(p)-half,0.0))` +
  `min(max(p.x,max(p.y,p.z))+half.x, 0.0)` for the interior)
- `Cylinder()`, `Cone()` etc.: same shape logic, new types

**Keep `Mesh` in Lua** — it currently warns "horribly inefficient"; for now just
remove it or keep a stub that returns an error ("Mesh not supported in GPU mode").

---

### Step 6 — tessellation adaptor

`tessellate()` in `lib.rs` currently uses `ObjectAdaptor` wrapping `Box<dyn Object<Float>>`.
Replace with a new adaptor wrapping `Box<dyn Primitive>`:

```rust
struct TessAdaptor {
    object: Box<dyn Primitive>,
    // cache bbox as implicit3d::BoundingBox for the ImplicitFunction trait
    bbox: implicit3d::BoundingBox<f64>,
}

impl ImplicitFunction<f64> for TessAdaptor {
    fn bbox(&self) -> &implicit3d::BoundingBox<f64> { &self.bbox }
    fn value(&self, p: &na::Point3<f64>) -> f64 {
        self.object.eval([p.x as f32, p.y as f32, p.z as f32]) as f64
    }
    fn normal(&self, p: &na::Point3<f64>) -> na::Vector3<f64> {
        // finite differences via self.object.eval()
        let e = 0.001f32;
        let [x,y,z] = [p.x as f32, p.y as f32, p.z as f32];
        let dx = self.object.eval([x+e,y,z]) - self.object.eval([x-e,y,z]);
        let dy = self.object.eval([x,y+e,z]) - self.object.eval([x,y-e,z]);
        let dz = self.object.eval([x,y,z+e]) - self.object.eval([x,y,z-e]);
        na::Vector3::new(dx as f64, dy as f64, dz as f64).normalize()
    }
}
```

`implicit3d` stays in `Cargo.toml` **only** because `tessellation` needs its
`BoundingBox` type. Everything else in the codebase is migrated off it.

---

### Step 7 — `web/main.js`: WebGL2 rendering loop

Replace the `render()` → `putImageData` block with:

```js
// --- WebGL2 setup (once at startup) ---
const gl = canvas.getContext('webgl2');
// full-screen quad: two triangles covering clip space
const verts = new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]);
const buf = gl.createBuffer();
gl.bindBuffer(gl.ARRAY_BUFFER, buf);
gl.bufferData(gl.ARRAY_BUFFER, verts, gl.STATIC_DRAW);

let program = null;   // compiled shader program
let uResolution, uTransform;

function compileShader(src) {
    // compile vertex + fragment shaders, link program
    // vertex shader: just `gl_Position = vec4(aPos, 0, 1);`
    // store uResolution, uTransform uniform locations
}

// --- Called after run_script() succeeds ---
function onNewObject() {
    const src = wasm.get_shader_source();
    if (src) {
        program = compileShader(src);
        startRenderLoop();
    }
}

// --- rAF loop ---
let rafId = null;
function startRenderLoop() {
    if (rafId) cancelAnimationFrame(rafId);
    function frame() {
        if (!program) return;
        gl.useProgram(program);
        gl.uniform2f(uResolution, canvas.width, canvas.height);
        gl.uniformMatrix4fv(uTransform, false, wasm.get_world_transform());
        gl.drawArrays(gl.TRIANGLES, 0, 6);
        rafId = requestAnimationFrame(frame);
    }
    rafId = requestAnimationFrame(frame);
}

// --- Mouse drag: same events, different effect ---
// rotate/pan still call wasm.rotate(dx,dy) / wasm.pan(dx,dy)
// no explicit re-render needed — rAF loop picks up new matrix next frame
```

The preview `<canvas>` in `index.html` needs no change other than removing
`width`/`height` attributes (let CSS control size; read `canvas.clientWidth` in JS).

---

### Step 8 — `Cargo.toml` cleanup

Remove:
```toml
implicit3d = "0.16"   # ← keep only if tessellation still needs BoundingBox
                       #   (see Step 6); remove entirely if tessellation crate
                       #   is also updated to not depend on it
```

No new dependencies needed — `nalgebra` is already present.

If `IndexSet` is desired for `GlslCtx.helpers`, add:
```toml
indexmap = "2"
```
Otherwise a plain `Vec<String>` with `.iter().any(|h| h == src)` is fine.

---

### Cargo.toml note on `implicit3d`

Check whether `tessellation`'s `ImplicitFunction` trait bounds actually enforce
`implicit3d::BoundingBox` or just use a locally defined type. If `tessellation`
owns its own `BoundingBox` (re-exported or redefined), `implicit3d` can be dropped
entirely. If it re-exports `implicit3d::BoundingBox`, keep `implicit3d` but mark
it `optional` and only pull it in for the tessellation path.

---

### Implementation order

1. `src/primitive.rs` — trait, Bbox, GlslCtx (no deps, testable in isolation)
2. `src/primitives/*.rs` — implement all types, write unit tests:
   - `Sphere::new(1.0).eval([0,0,0])` == `-1.0`
   - `Sphere::new(1.0).expression(...)` contains `length`
   - `Union` of two spheres has `min(` in output
   - Smooth union output contains `smin` helper exactly once
3. `src/shader.rs` + `src/renderer.glsl` — build and test shader string
4. `src/lib.rs` — swap API, delete render.rs
5. `src/luascad.rs` — swap primitive type throughout
6. `web/main.js` — WebGL2 loop
7. Manual smoke test: `wasm-pack build --target web`, open in browser
8. Tune camera / lighting in `renderer.glsl`

---

### Open questions to resolve at implementation time

1. **Camera distance**: currently derived from `object_width()` in render.rs — pass
   as a uniform `iObjectWidth` or compute `ro.z` in Rust and pass as `iCameraZ`.
2. **Non-uniform scale**: decide whether to support it (incorrect SDF metric) or
   restrict the Lua `Scale()` API to uniform scale only.
3. **Mesh primitive**: remove from Lua API for now, or keep with an error message.
4. **`implicit3d` removal**: check `tessellation` source to confirm whether its
   `BoundingBox` is self-contained or re-exported from `implicit3d`.
5. **`indexmap` vs plain Vec** for GlslCtx helpers deduplication.
6. **WebGL2 availability**: add a fallback message if `getContext('webgl2')` returns
   null (old/mobile browsers).
