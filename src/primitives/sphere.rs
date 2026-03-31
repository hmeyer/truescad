use crate::primitive::{Bbox, GlslCtx, Primitive};

#[derive(Clone)]
pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Sphere { radius }
    }
}

impl Primitive for Sphere {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let d = ctx.fresh_float();
        ctx.push(format!("float {d} = length({p}) - {:.8};", self.radius));
        d
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        (x * x + y * y + z * z).sqrt() - self.radius
    }
    fn bbox(&self) -> Bbox {
        let r = self.radius;
        Bbox { min: [-r, -r, -r], max: [r, r, r] }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}
