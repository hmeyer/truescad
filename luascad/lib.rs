#[macro_use]
extern crate hlua;
extern crate implicit3d;
extern crate nalgebra;

pub mod lobject;
pub mod lobject_vector;
pub mod luascad;
pub mod printbuffer;
pub mod sandbox;

pub use self::luascad::eval;

type Float = f64;
const EPSILON: f64 = std::f64::EPSILON;
