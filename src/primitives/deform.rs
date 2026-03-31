use crate::primitive::{Bbox, GlslCtx, Primitive};

// ── Bender ────────────────────────────────────────────────────────────────────
// Bends the XZ plane based on the Y coordinate.
// angle = p.y / width  (radians)
// p' = (x*cos(angle) - z*sin(angle),  x*sin(angle) + z*cos(angle),  y)

#[derive(Clone)]
pub struct Bender {
    inner: Box<dyn Primitive>,
    pub width: f32,
}

impl Bender {
    pub fn new(inner: Box<dyn Primitive>, width: f32) -> Self {
        Bender { inner, width }
    }
}

impl Primitive for Bender {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let p1 = ctx.fresh_point();
        let w = self.width;
        ctx.push(format!(
            "float _by_{p1} = {p}.y / {w:.8};\
            \nvec3 {p1} = vec3(\
            {p}.x * cos(_by_{p1}) - {p}.z * sin(_by_{p1}), \
            {p}.x * sin(_by_{p1}) + {p}.z * cos(_by_{p1}), \
            {p}.y);"
        ));
        self.inner.expression(&p1, ctx)
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        let angle = y / self.width;
        let (s, c) = angle.sin_cos();
        self.inner.eval([x * c - z * s, x * s + z * c, y])
    }
    fn bbox(&self) -> Bbox {
        // After bending: new_z = old_y, and new_x/new_y = rotation of old_x/old_z.
        // Max |new_x| = max |new_y| ≤ sqrt(x_half² + z_half²).
        let b = self.inner.bbox();
        let x_half = b.min[0].abs().max(b.max[0].abs());
        let z_half = b.min[2].abs().max(b.max[2].abs());
        let r = (x_half * x_half + z_half * z_half).sqrt();
        Bbox { min: [-r, -r, b.min[1]], max: [r, r, b.max[1]] }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}

// ── Twister ───────────────────────────────────────────────────────────────────
// Twists the XY plane around the Z axis.
// angle = p.z / height * 2π
// p' = (cos(angle)*x - sin(angle)*y,  sin(angle)*x + cos(angle)*y,  z)

#[derive(Clone)]
pub struct Twister {
    inner: Box<dyn Primitive>,
    pub height: f32,
}

impl Twister {
    pub fn new(inner: Box<dyn Primitive>, height: f32) -> Self {
        Twister { inner, height }
    }
}

impl Primitive for Twister {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let p1 = ctx.fresh_point();
        let h = self.height;
        ctx.push(format!(
            "float _angle_{p1} = {p}.z / {h:.8} * 6.28318530718;\
            \nfloat _c_{p1} = cos(_angle_{p1}), _s_{p1} = sin(_angle_{p1});\
            \nvec3 {p1} = vec3(_c_{p1}*{p}.x - _s_{p1}*{p}.y, _s_{p1}*{p}.x + _c_{p1}*{p}.y, {p}.z);"
        ));
        self.inner.expression(&p1, ctx)
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        let angle = z / self.height * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        self.inner.eval([c * x - s * y, s * x + c * y, z])
    }
    fn bbox(&self) -> Bbox {
        let b = self.inner.bbox();
        let r = b.min[0].abs().max(b.max[0].abs())
            .max(b.min[1].abs()).max(b.max[1].abs());
        Bbox { min: [-r, -r, b.min[2]], max: [r, r, b.max[2]] }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}
