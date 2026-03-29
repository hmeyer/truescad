pub mod luascad;
pub mod render;

use std::cell::RefCell;

use implicit3d::{Object, PrimitiveParameters};
use js_sys::{Uint8Array, Uint8ClampedArray};
use tessellation::{ImplicitFunction, ManifoldDualContouring};
use wasm_bindgen::prelude::*;

type Float = f64;
const EPSILON: f64 = std::f64::EPSILON;

const TESSELLATION_RESOLUTION: Float = 0.12;
const TESSELLATION_ERROR: Float = 2.0;
const FADE_RANGE: Float = 0.1;
const R_MULTIPLIER: Float = 1.0;

struct AppState {
    renderer: render::Renderer,
    object: Option<Box<dyn Object<Float>>>,
}

impl AppState {
    fn new() -> Self {
        AppState { renderer: render::Renderer::new(), object: None }
    }
}

thread_local! {
    static STATE: RefCell<AppState> = RefCell::new(AppState::new());
}

/// Evaluate a Lua script. Returns a JS object `{output: string, error: string|null}`.
#[wasm_bindgen]
pub fn run_script(code: &str) -> JsValue {
    match luascad::eval(code) {
        Ok((output, maybe_obj)) => {
            STATE.with(|s| {
                let mut state = s.borrow_mut();
                let obj = maybe_obj.map(|mut o| {
                    o.set_parameters(&PrimitiveParameters {
                        fade_range: FADE_RANGE,
                        r_multiplier: R_MULTIPLIER,
                    });
                    o
                });
                state.renderer.set_object(obj.as_ref().map(|o| o.clone_box()));
                state.object = obj;
            });
            let result = js_sys::Object::new();
            js_sys::Reflect::set(&result, &"output".into(), &output.into()).unwrap();
            js_sys::Reflect::set(&result, &"error".into(), &JsValue::NULL).unwrap();
            result.into()
        }
        Err(e) => {
            let result = js_sys::Object::new();
            js_sys::Reflect::set(&result, &"output".into(), &"".into()).unwrap();
            js_sys::Reflect::set(&result, &"error".into(), &e.to_string().into()).unwrap();
            result.into()
        }
    }
}

/// Render the current object into a pixel buffer ready for `ctx.putImageData`.
#[wasm_bindgen]
pub fn render(width: u32, height: u32) -> Uint8ClampedArray {
    let mut buf = vec![0u8; (width * height * 4) as usize];
    STATE.with(|s| {
        s.borrow().renderer.draw_on_buf(&mut buf, width as i32, height as i32);
    });
    Uint8ClampedArray::from(buf.as_slice())
}

/// Rotate the view by a screen-space delta.
#[wasm_bindgen]
pub fn rotate(dx: f64, dy: f64) {
    STATE.with(|s| s.borrow_mut().renderer.rotate_from_screen(dx, dy));
}

/// Pan the view by a screen-space delta.
#[wasm_bindgen]
pub fn pan(dx: f64, dy: f64) {
    STATE.with(|s| s.borrow_mut().renderer.translate_from_screen(dx, dy));
}

struct ObjectAdaptor {
    object: Box<dyn Object<Float>>,
}

impl ImplicitFunction<Float> for ObjectAdaptor {
    fn value(&self, p: &nalgebra::Point3<Float>) -> Float {
        self.object.approx_value(p, TESSELLATION_RESOLUTION)
    }
    fn normal(&self, p: &nalgebra::Point3<Float>) -> nalgebra::Vector3<Float> {
        self.object.normal(p)
    }
}

/// Tessellate the current object and return binary STL bytes.
/// Used for both the mesh view (Three.js STLLoader) and file export.
/// Returns `null` if no object is loaded.
#[wasm_bindgen]
pub fn tessellate() -> Option<Uint8Array> {
    STATE.with(|s| {
        let state = s.borrow();
        let obj = state.object.as_ref()?.clone_box();
        let adaptor = ObjectAdaptor { object: obj };
        let mut mdc = ManifoldDualContouring::new(
            &adaptor,
            TESSELLATION_RESOLUTION,
            TESSELLATION_ERROR,
        );
        let mesh = mdc.tessellate()?;

        let triangles: Vec<stl_io::Triangle> = mesh
            .faces
            .iter()
            .map(|face| {
                let v: [[f32; 3]; 3] = [
                    [mesh.vertices[face[0]][0] as f32, mesh.vertices[face[0]][1] as f32, mesh.vertices[face[0]][2] as f32],
                    [mesh.vertices[face[1]][0] as f32, mesh.vertices[face[1]][1] as f32, mesh.vertices[face[1]][2] as f32],
                    [mesh.vertices[face[2]][0] as f32, mesh.vertices[face[2]][1] as f32, mesh.vertices[face[2]][2] as f32],
                ];
                let a = nalgebra::Vector3::new(v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]);
                let b = nalgebra::Vector3::new(v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]);
                let n = a.cross(&b).normalize();
                stl_io::Triangle {
                    normal: stl_io::Normal::new([n.x, n.y, n.z]),
                    vertices: [
                        stl_io::Vertex::new(v[0]),
                        stl_io::Vertex::new(v[1]),
                        stl_io::Vertex::new(v[2]),
                    ],
                }
            })
            .collect();

        let mut buf = Vec::new();
        stl_io::write_stl(&mut buf, triangles.iter()).ok()?;
        Some(Uint8Array::from(buf.as_slice()))
    })
}
