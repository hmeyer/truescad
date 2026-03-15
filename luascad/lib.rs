pub extern crate implicit3d;
pub extern crate nalgebra;

mod lobject;
pub mod luascad;

#[cfg(test)]
mod tests;

pub use self::luascad::eval;

type Float = f64;
const EPSILON: f64 = std::f64::EPSILON;
