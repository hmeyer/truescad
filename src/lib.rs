pub mod luascad;
pub mod render;

pub use luascad::eval;

type Float = f64;
const EPSILON: f64 = std::f64::EPSILON;
