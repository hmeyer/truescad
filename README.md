# truescad
[![CI](https://github.com/hmeyer/truescad/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/hmeyer/truescad/actions/workflows/ci.yml)
[![Deploy](https://github.com/hmeyer/truescad/actions/workflows/deploy.yml/badge.svg?branch=main)](https://hmeyer.github.io/truescad/)

**[▶ Open TrueScad in your browser](https://hmeyer.github.io/truescad/)**

TrueScad is a Lua-scripted CAD tool similar to [OpenSCAD](http://www.openscad.org/), running entirely in the browser as a WebAssembly app. Like [ImplicitCAD](http://www.implicitcad.org/), it uses implicit functions to represent geometry, offering precise surfaces and smooth rounded CSG.

![Accurate geometry view](doc/true_view.png)
![Tessellated mesh](doc/tessellated.png)

## Using the app

Write a Lua script in the left panel using the primitives below, then:

- **Run** — evaluates the script and shows a ray-marched preview
- **Mesh** — tessellates the geometry and shows a 3D mesh (drag to rotate)
- **Export STL** — downloads the tessellated mesh as an STL file

Drag on the preview canvas to **rotate** (left button) or **pan** (right button).

## Lua API

### Primitives

```lua
Sphere(radius)
Box(x, y, z, smooth?)           -- smooth rounds the edges
Cylinder({l=length, r=radius, s=smooth?})
Cylinder({l=length, r1=r1, r2=r2, s=smooth?})  -- tapered
iCylinder(radius)               -- infinite cylinder
iCone(slope)                    -- infinite cone
PlaneX(d)  PlaneNegX(d)         -- half-spaces
PlaneY(d)  PlaneNegY(d)
PlaneZ(d)  PlaneNegZ(d)
PlaneHessian({nx,ny,nz}, p)
Plane3Points({x,y,z}, {x,y,z}, {x,y,z})
```

### Boolean operations

```lua
Union({obj, ...}, smooth?)
Intersection({obj, ...}, smooth?)
Difference({obj, ...}, smooth?)  -- first minus the rest
```

### Transformations (method syntax)

```lua
obj:translate(x, y, z)
obj:rotate(x, y, z)      -- Euler angles in radians
obj:scale(x, y, z)
obj:clone()
```

### Deformations

```lua
Bend(obj, width)
Twist(obj, height)
```

### Output

```lua
build(obj)   -- sets the object to render/export
print(...)   -- output appears in the log panel
```

### Example

```lua
cube   = Box(1, 1, 1, 0.3)
sphere = Sphere(0.5)
result = Difference({cube, sphere}, 0.3)
result = result:scale(15, 15, 15)
build(result)
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/) — `cargo install wasm-pack`
- Node.js 18+

### Build and run locally

```bash
# Install JS dependencies (once)
npm install

# Build the Rust WASM module
wasm-pack build --target web

# Bundle the frontend and start the dev server
npm run serve
# → http://localhost:8080
```

After changing Rust code, re-run `wasm-pack build --target web` and refresh the browser.
After changing JS/CSS in `web/`, the dev server rebuilds automatically.

### Run Rust tests

```bash
cargo test
```

### Production build

```bash
wasm-pack build --target web --release
npm run build:release
# Output in dist/
```

## Architecture

- **`src/luascad.rs`** — Lua scripting engine ([piccolo](https://github.com/kyren/piccolo)), exposes all geometry primitives
- **`src/shader.rs`** — builds the GLSL fragment shader for GPU ray-marching
- **`src/lib.rs`** — `wasm-bindgen` API surface (`eval`, `render`, `rotate`, `pan`, `tessellate`)
- **`web/`** — vanilla JS frontend: CodeMirror 6 editor, Three.js mesh view
- **`build.mjs`** / **`serve.mjs`** — esbuild-based build and dev server
