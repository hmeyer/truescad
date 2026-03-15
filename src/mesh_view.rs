use super::Float;
use kiss3d::light::Light;
use kiss3d::window::Window;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Once};
use tessellation::Mesh;

#[derive(Clone)]
struct SingletonWindow {
    inner: Arc<Mutex<Window>>,
}

fn singleton_window() -> SingletonWindow {
    static mut SINGLETON: *const SingletonWindow = std::ptr::null();
    static ONCE: Once = Once::new();

    unsafe {
        ONCE.call_once(|| {
            let window = SingletonWindow {
                inner: Arc::new(Mutex::new(Window::new("MeshView"))),
            };
            SINGLETON = Box::into_raw(Box::new(window));
        });
        (*SINGLETON).clone()
    }
}

pub fn show_mesh(mesh: &Mesh<Float>) {
    let window_mutex = singleton_window();
    let mut window = window_mutex.inner.lock().unwrap();

    let scale = kiss3ddeps::Vector3::new(1.0, 1.0, 1.0);
    let mut object_node = window.add_mesh(tessellation_to_kiss3d_mesh(mesh), scale);

    object_node.set_color(1.0, 1.0, 0.0);

    window.set_light(Light::StickToCamera);

    while pollster::block_on(window.render()) {}
    window.remove_node(&mut object_node);
}

fn tessellation_to_kiss3d_mesh(mesh: &Mesh<Float>) -> Rc<RefCell<kiss3d::resource::GpuMesh>> {
    let mut na_verts = Vec::new();
    let mut na_faces = Vec::new();
    for face in &mesh.faces {
        let i = na_verts.len();
        na_faces.push(kiss3ddeps::Point3::new(
            i as u16,
            (i + 1) as u16,
            (i + 2) as u16,
        ));
        for index in face.iter() {
            let p = &mesh.vertices[*index];
            na_verts.push(kiss3ddeps::Point3::new(
                p[0] as f32,
                p[1] as f32,
                p[2] as f32,
            ));
        }
    }
    Rc::new(RefCell::new(kiss3d::resource::GpuMesh::new(
        na_verts, na_faces, None, None, true,
    )))
}
