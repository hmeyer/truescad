pub mod editor;
pub mod luascad;
pub mod menu;
pub mod mesh_view;
pub mod object_widget;
pub mod render;
pub mod settings;
pub mod window;

pub use luascad::eval;

type Float = f64;
const EPSILON: f64 = std::f64::EPSILON;
