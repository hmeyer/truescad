# TrueScad WASM Webapp — Status

## All phases complete (v0.9.0)

### Architecture

```
Lua script
  └─► piccolo evaluates
        ├─► Box<dyn Primitive>  (in-tree primitive module)
        │     ├─► expression() → GLSL SDF → fragment shader
        │     └─► eval()       → dual contouring → binary STL
        └─► AppState { object, world_transform, object_width }

Browser
  ├─► WebGL2 rAF loop
  │     fragment shader: ray-marches the SDF on GPU, real-time
  └─► Three.js mesh view
        tessellate() → ManifoldDualContouring → STL → OrbitControls
```

### Source layout

```
src/
  lib.rs           WASM API: run_script, get_shader_source, get_world_transform,
                             get_object_width, rotate, pan, tessellate
  luascad.rs       Lua→Primitive bridge (piccolo scripting engine)
  primitive.rs     Primitive trait, Bbox, GlslCtx
  primitives/
    sphere.rs      Sphere
    planes.rs      PlaneX/Y/Z/NegX/NegY/NegZ, NormalPlane
    cylinder.rs    InfCylinder
    cone.rs        InfCone
    csg.rs         Union, Intersection, Difference  (smooth CSG via IQ polynomial smin/smax)
    transforms.rs  Translate, Rotate, Scale
    deform.rs      Bender, Twister
  shader.rs        build_fragment_shader() — assembles GLSL from Primitive tree
  renderer.glsl    Ray-marching template: calcNormal, softShadow, void main()
web/
  main.js          WebGL2 setup, rAF loop, CodeMirror 6 editor, Three.js mesh view
  index.html
  main.css
```

### Lua API

| Function | Description |
|---|---|
| `Sphere(r)` | Sphere of radius r |
| `Box(x, y, z, smooth)` | Rounded box (6-plane intersection) |
| `Cylinder({l, r, s})` | Cylinder along Z, optional smooth caps |
| `Cylinder({l, r1, r2, s})` | Truncated cone |
| `PlaneX/Y/Z/NegX/NegY/NegZ(d)` | Axis-aligned half-space |
| `PlaneHessian(n, p)` | Plane by normal + offset |
| `Plane3Points(a, b, c)` | Plane through three points |
| `Union({...}, smooth)` | Smooth or hard union |
| `Intersection({...}, smooth)` | Smooth or hard intersection |
| `Difference({...}, smooth)` | Smooth or hard difference |
| `obj:translate(x,y,z)` | Translation |
| `obj:rotate(rx,ry,rz)` | Euler rotation |
| `obj:scale(sx,sy,sz)` | Non-uniform scale |
| `Bend(obj, width)` | XZ bend deformation |
| `Twist(obj, height)` | Z-axis twist deformation |
| `build(obj)` | Register object for rendering/tessellation |

Global constants: `pi`, `tau`.

### Design notes

- **Smooth SDF**: IQ polynomial `smin`/`smax` add up to `k/4` to the SDF result,
  shifting the zero-crossing inward. Keep `k` small relative to feature size
  (e.g. `k=0.02` for a unit object). Large `k` (≥ 20% of object size) renders
  as invisible — all rays return positive SDF.

- **Camera**: fixed focal length 1.5 (~44° FOV), camera Z = `object_width × 2.5`.

- **Scale + SDF**: non-uniform scale is supported but the SDF metric becomes
  approximate. Lighting and shadow softness may look slightly off for extreme
  non-uniform scales; shape and surface are correct.

- **tessellation 0.11**: `ImplicitFunction` no longer requires `bbox()` — bounds
  are auto-detected via `find_bounds()`. `implicit3d` crate removed entirely.

### Build

```sh
# First build
wasm-pack build --target web
npm run build

# Dev (watches Rust + JS, auto-rebuilds)
npm run dev

# Release build (minified)
wasm-pack build --target web --release
npm run build:release

# Browser smoke test
node tests/browser.mjs [--screenshot]
```
