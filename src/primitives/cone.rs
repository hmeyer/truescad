use crate::primitive::{Bbox, GlslCtx, Primitive};

const INF: f32 = 1e10;

/// Infinite cone along the Z-axis with a given slope and apex offset.
/// SDF matches implicit3d::Cone: (length(p.xy) - |slope*(p.z+offset)|) / sqrt(slope²+1)
#[derive(Clone)]
pub struct InfCone {
    pub slope: f32,
    pub offset: f32,
    dm: f32, // 1 / sqrt(slope² + 1)
}

impl InfCone {
    pub fn new(slope: f32, offset: f32) -> Self {
        let dm = 1.0 / (slope * slope + 1.0f32).sqrt();
        InfCone { slope, offset, dm }
    }
}

impl Primitive for InfCone {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let r = ctx.fresh_float();
        let d = ctx.fresh_float();
        ctx.push(format!("float {r} = length({p}.xy);"));
        ctx.push(format!(
            "float {d} = ({r} - abs({:.8} * ({p}.z + {:.8}))) * {:.8};",
            self.slope, self.offset, self.dm
        ));
        d
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        let r = (x * x + y * y).sqrt();
        (r - (self.slope * (z + self.offset)).abs()) * self.dm
    }
    fn bbox(&self) -> Bbox {
        Bbox { min: [-INF, -INF, -INF], max: [INF, INF, INF] }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}
