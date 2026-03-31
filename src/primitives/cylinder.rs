use crate::primitive::{Bbox, GlslCtx, Primitive};

const INF: f32 = 1e10;

/// Infinite cylinder along the Z-axis.
#[derive(Clone)]
pub struct InfCylinder {
    pub radius: f32,
}

impl InfCylinder {
    pub fn new(radius: f32) -> Self {
        InfCylinder { radius }
    }
}

impl Primitive for InfCylinder {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let d = ctx.fresh_float();
        ctx.push(format!("float {d} = length({p}.xy) - {:.8};", self.radius));
        d
    }
    fn eval(&self, [x, y, _z]: [f32; 3]) -> f32 {
        (x * x + y * y).sqrt() - self.radius
    }
    fn bbox(&self) -> Bbox {
        let r = self.radius;
        Bbox { min: [-r, -r, -INF], max: [r, r, INF] }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}
