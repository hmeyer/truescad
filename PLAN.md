# TrueScad WASM Webapp Plan

Convert TrueScad from a GTK desktop app to a WASM webapp hosted on GitHub Pages.
The GTK desktop app is being abandoned in favor of a web-first approach.

## Architecture

- **Rust** compiles to a `cdylib` WASM module via `wasm-pack`, exposing a `wasm-bindgen` API
- **Vanilla JS** + HTML/CSS handles all UI
- **CodeMirror 6** (bundled via npm/esbuild) for the Lua editor
- **`<canvas>`** receives pixel data via `putImageData` directly from the Rust renderer
- Rendering is **on-demand**: triggered by Run and drag events only

## Crate changes

| Item | Action |
|---|---|
| `src/window.rs`, `editor.rs`, `menu.rs`, `object_widget.rs`, `settings.rs`, `mesh_view.rs` | Delete |
| `src/render.rs`, `src/lib.rs` | Keep — pure math, WASM-ready |
| `src/main.rs` | Replace with `wasm-bindgen` entry point |
| Root `Cargo.toml` | Remove gtk/sourceview4/gdk/glib/cairo-rs/kiss3d/rayon/dirs/pollster; add `wasm-bindgen`, `web-sys` |
| `kiss3ddeps/` | Delete (only existed for kiss3d) |
| `luascad/` | Deleted — folded into `src/luascad.rs` |

## Views

### Ray-march preview (primary)
- Driven by the existing `Renderer` in `src/render.rs` — writes to `&mut [u8]` pixel buffer
- WASM exposes `render(width, height) -> Uint8ClampedArray`
- JS feeds the buffer into a `<canvas>` via `ctx.putImageData(new ImageData(arr, w, h))`
- Mouse drag on canvas → `rotate(dx, dy)` / `pan(dx, dy)` → re-render
- Fast feedback during editing; re-triggered on every Run

### Mesh view (secondary, like the current kiss3d window)
- Driven by `tessellation::ManifoldDualContouring` on the Rust side
- WASM exposes `tessellate() -> Uint8Array` (binary STL bytes)
- JS uses **Three.js** `STLLoader` to parse the bytes and display the mesh in a WebGL canvas
- Three.js `OrbitControls` for rotate/pan/zoom of the mesh
- Same `Uint8Array` is reused for the "Export STL" download — no duplication
- Tessellation can be slow; triggered explicitly by a **[Mesh]** button, not on every Run

### UI layout

```
┌──────────────────────────────────────────────────────────────┐
│  [Run]  [Mesh]  [Export STL]                    (toolbar)    │
├─────────────────────────┬────────────────────────────────────┤
│                         │  [Preview] [Mesh]  ← tab toggle    │
│  CodeMirror 6           │  ┌──────────────────────────────┐  │
│  (Lua editor)           │  │  <canvas>                    │  │
│                         │  │  ray-march OR Three.js mesh  │  │
│                         │  └──────────────────────────────┘  │
├─────────────────────────┴────────────────────────────────────┤
│  output / error log                             (bottom)     │
└──────────────────────────────────────────────────────────────┘
```

The right panel switches between the ray-march `<canvas>` and the Three.js `<canvas>` via the tab toggle. Three.js is loaded from npm alongside CodeMirror 6, bundled by esbuild.

## Frontend layout

```
index.html
main.js          ← wires CM6, canvas drag, buttons → WASM calls
main.css
pkg/             ← wasm-pack output (truescad.wasm + JS bindings)
node_modules/    ← CM6 + Three.js (dev only, bundled by esbuild)
```

## Rust WASM API (updated)

```rust
#[wasm_bindgen] pub fn eval(code: &str) -> EvalResult
#[wasm_bindgen] pub fn render(width: u32, height: u32) -> Uint8ClampedArray
#[wasm_bindgen] pub fn rotate(dx: f64, dy: f64)
#[wasm_bindgen] pub fn pan(dx: f64, dy: f64)
#[wasm_bindgen] pub fn tessellate() -> Uint8Array   // binary STL — used for mesh view AND export
```

`tessellate()` replaces the separate `export_stl()` — same output, dual purpose.

## Phase order

1. **Strip root crate** — delete GTK files, rewrite `Cargo.toml`
2. **`wasm-bindgen` API** — implement `src/lib.rs` with `eval`/`render`/`rotate`/`pan`/`tessellate`
3. **Verify WASM builds** — `wasm-pack build --target web`
4. **Frontend scaffold** — `index.html` + `main.js` + CodeMirror 6 + Three.js via esbuild
5. **Wire JS ↔ WASM** — Run button, ray-march canvas, Mesh button + Three.js STL view, Export download
6. **GitHub Actions** — npm install + esbuild bundle + `wasm-pack build --release` + deploy `dist/` to `gh-pages`

## Status

- [x] piccolo migration complete (`luascad` uses piccolo 0.3, no native C deps)
- [x] `luascad` subcrate folded into `src/luascad.rs` (workspace simplified)
- [ ] Phase 1: Strip root crate
- [ ] Phase 2: wasm-bindgen API
- [ ] Phase 3: Verify WASM builds
- [ ] Phase 4: Frontend scaffold
- [ ] Phase 5: Wire JS ↔ WASM
- [ ] Phase 6: GitHub Actions + GitHub Pages
