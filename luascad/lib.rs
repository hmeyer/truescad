pub extern crate implicit3d;

mod luascad;

pub use luascad::{eval, EvalResult};

type Float = f64;
const EPSILON: f64 = f64::EPSILON;
