# TrueScad WASM Webapp Plan

Convert TrueScad from a GTK desktop app to a WASM webapp hosted on GitHub Pages.
The GTK desktop app is being abandoned in favor of a web-first approach.

## Architecture

- **Rust** compiles to a `cdylib` WASM module via `wasm-pack`, exposing a `wasm-bindgen` API
- **Vanilla JS** + HTML/CSS handles all UI
- **CodeMirror 6** (bundled via npm/esbuild) for the Lua editor
- **`<canvas>`** receives pixel data via `putImageData` directly from the Rust renderer
- Rendering is **on-demand**: triggered by Run and drag events only

## Rust WASM API

```rust
#[wasm_bindgen] pub fn eval(code: &str) -> EvalResult
#[wasm_bindgen] pub fn render(width: u32, height: u32) -> Uint8ClampedArray
#[wasm_bindgen] pub fn rotate(dx: f64, dy: f64)
#[wasm_bindgen] pub fn pan(dx: f64, dy: f64)
#[wasm_bindgen] pub fn export_stl() -> Uint8Array
```

`Uint8ClampedArray` slots directly into `new ImageData(arr, w, h)` → `ctx.putImageData()` with no copy.

## Crate changes

| Item | Action |
|---|---|
| `src/window.rs`, `editor.rs`, `menu.rs`, `object_widget.rs`, `settings.rs`, `mesh_view.rs` | Delete |
| `src/render.rs`, `src/lib.rs` | Keep — pure math, WASM-ready |
| `src/main.rs` | Replace with `wasm-bindgen` entry point |
| Root `Cargo.toml` | Remove gtk/sourceview4/gdk/glib/cairo-rs/kiss3d/rayon/dirs/pollster; add `wasm-bindgen`, `web-sys` |
| `kiss3ddeps/` | Delete (only existed for kiss3d) |
| `luascad/` | Keep as-is (piccolo — WASM-ready) |

## Frontend layout

```
index.html
main.js          ← wires CM6, canvas drag, buttons → WASM calls
main.css
pkg/             ← wasm-pack output (truescad.wasm + JS bindings)
node_modules/    ← CM6 (dev only, bundled by esbuild)
```

## Phase order

1. **Strip root crate** — delete GTK files, rewrite `Cargo.toml`
2. **`wasm-bindgen` API** — implement `src/lib.rs` wrapping `render.rs` + `luascad::eval`
3. **Verify WASM builds** — `wasm-pack build --target web`
4. **Frontend scaffold** — `index.html` + `main.js` + CodeMirror 6
5. **Wire JS ↔ WASM** — Run button, drag-to-rotate, STL export download
6. **GitHub Actions** — `wasm-pack build --release` + deploy `dist/` to `gh-pages`

## Status

- [x] piccolo migration complete (`luascad` uses piccolo 0.3, no native C deps)
- [ ] Phase 1: Strip root crate
- [ ] Phase 2: wasm-bindgen API
- [ ] Phase 3: Verify WASM builds
- [ ] Phase 4: Frontend scaffold
- [ ] Phase 5: Wire JS ↔ WASM
- [ ] Phase 6: GitHub Actions + GitHub Pages
