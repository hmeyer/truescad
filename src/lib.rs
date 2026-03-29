pub mod luascad;
pub mod primitive;
pub mod primitives;
pub mod shader;

use std::cell::RefCell;

use js_sys::Uint8Array;
use nalgebra as na;
use tessellation::{ImplicitFunction, ManifoldDualContouring};
use wasm_bindgen::prelude::*;

use primitive::Primitive;

const TESSELLATION_RESOLUTION: f64 = 0.12;
const TESSELLATION_ERROR: f64 = 2.0;

struct AppState {
    object: Option<Box<dyn Primitive>>,
    world_transform: na::Matrix4<f32>,
    object_width: f32,
}

impl AppState {
    fn new() -> Self {
        AppState {
            object: None,
            world_transform: na::Matrix4::identity(),
            object_width: 1.0,
        }
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
                state.object_width = maybe_obj
                    .as_ref()
                    .map(|o| o.bbox().width())
                    .unwrap_or(1.0)
                    .max(0.001);
                state.world_transform = na::Matrix4::identity();
                state.object = maybe_obj;
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

/// Returns the GLSL fragment shader source for the current scene, or null if no object is loaded.
#[wasm_bindgen]
pub fn get_shader_source() -> Option<String> {
    STATE.with(|s| {
        let state = s.borrow();
        let obj = state.object.as_ref()?;
        Some(shader::build_fragment_shader(obj.as_ref()))
    })
}

/// Returns the current world transform as a flat 16-element f32 array (column-major).
#[wasm_bindgen]
pub fn get_world_transform() -> Vec<f32> {
    STATE.with(|s| {
        let state = s.borrow();
        state.world_transform.as_slice().to_vec()
    })
}

/// Returns the camera Z distance derived from the object size.
#[wasm_bindgen]
pub fn get_object_width() -> f32 {
    STATE.with(|s| s.borrow().object_width)
}

/// Rotate the view by a screen-space delta.
#[wasm_bindgen]
pub fn rotate(dx: f64, dy: f64) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let euler =
            na::Rotation3::from_euler_angles(dy as f32, dx as f32, 0.).to_homogeneous();
        state.world_transform = euler * state.world_transform;
    });
}

/// Pan the view by a screen-space delta.
#[wasm_bindgen]
pub fn pan(dx: f64, dy: f64) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let w = state.object_width;
        let t = na::Vector3::new(-dx as f32 * w, dy as f32 * w, 0.0);
        state.world_transform = state.world_transform.append_translation(&t);
    });
}

// ── Tessellation ──────────────────────────────────────────────────────────────

struct TessAdaptor {
    object: Box<dyn Primitive>,
}

impl ImplicitFunction<f64> for TessAdaptor {
    fn value(&self, p: &na::Point3<f64>) -> f64 {
        self.object.eval([p.x as f32, p.y as f32, p.z as f32]) as f64
    }
    fn normal(&self, p: &na::Point3<f64>) -> na::Vector3<f64> {
        let e = 0.001f32;
        let [x, y, z] = [p.x as f32, p.y as f32, p.z as f32];
        let dx = self.object.eval([x + e, y, z]) - self.object.eval([x - e, y, z]);
        let dy = self.object.eval([x, y + e, z]) - self.object.eval([x, y - e, z]);
        let dz = self.object.eval([x, y, z + e]) - self.object.eval([x, y, z - e]);
        na::Vector3::new(dx as f64, dy as f64, dz as f64).normalize()
    }
}

/// Tessellate the current object and return binary STL bytes.
/// Returns `null` if no object is loaded.
#[wasm_bindgen]
pub fn tessellate() -> Option<Uint8Array> {
    STATE.with(|s| {
        let state = s.borrow();
        let obj = state.object.as_ref()?.clone_box();
        let adaptor = TessAdaptor { object: obj };
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
                let v: [[f32; 3]; 3] = std::array::from_fn(|i| {
                    let vi = face[i];
                    [
                        mesh.vertices[vi][0] as f32,
                        mesh.vertices[vi][1] as f32,
                        mesh.vertices[vi][2] as f32,
                    ]
                });
                let a = na::Vector3::new(v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]);
                let b = na::Vector3::new(v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]);
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
