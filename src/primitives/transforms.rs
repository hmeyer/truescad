use crate::primitive::{Bbox, GlslCtx, Primitive};
use nalgebra as na;

const INF: f32 = 1e10;

// ── Translate ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Translate {
    inner: Box<dyn Primitive>,
    pub t: [f32; 3],
}

impl Translate {
    pub fn new(inner: Box<dyn Primitive>, t: [f32; 3]) -> Self {
        Translate { inner, t }
    }
}

impl Primitive for Translate {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let p1 = ctx.fresh_point();
        let [tx, ty, tz] = self.t;
        ctx.push(format!("vec3 {p1} = {p} - vec3({tx:.8}, {ty:.8}, {tz:.8});"));
        self.inner.expression(&p1, ctx)
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        self.inner.eval([x - self.t[0], y - self.t[1], z - self.t[2]])
    }
    fn bbox(&self) -> Bbox {
        let b = self.inner.bbox();
        Bbox {
            min: [b.min[0] + self.t[0], b.min[1] + self.t[1], b.min[2] + self.t[2]],
            max: [b.max[0] + self.t[0], b.max[1] + self.t[1], b.max[2] + self.t[2]],
        }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}

// ── Rotate ────────────────────────────────────────────────────────────────────
// Stores a 3×3 rotation matrix (nalgebra convention from_euler_angles(rx, ry, rz)).
// Evaluation: apply the rotation matrix to p, then evaluate inner.

#[derive(Clone)]
pub struct Rotate {
    inner: Box<dyn Primitive>,
    mat: [[f32; 3]; 3], // mat[row][col]
}

impl Rotate {
    pub fn new(inner: Box<dyn Primitive>, euler: [f32; 3]) -> Self {
        let r = na::Rotation3::from_euler_angles(euler[0], euler[1], euler[2]);
        let m = r.matrix();
        let mat = [
            [m[(0, 0)], m[(0, 1)], m[(0, 2)]],
            [m[(1, 0)], m[(1, 1)], m[(1, 2)]],
            [m[(2, 0)], m[(2, 1)], m[(2, 2)]],
        ];
        Rotate { inner, mat }
    }

    /// Compose an additional rotation on top of an existing Rotate.
    pub fn compose(mut self, euler: [f32; 3]) -> Self {
        let r_new = na::Rotation3::from_euler_angles(euler[0], euler[1], euler[2]);
        let m_new = r_new.matrix();
        // combined = m_new * self.mat  (apply self.mat first, then m_new)
        let a = self.mat;
        let b = [
            [m_new[(0, 0)], m_new[(0, 1)], m_new[(0, 2)]],
            [m_new[(1, 0)], m_new[(1, 1)], m_new[(1, 2)]],
            [m_new[(2, 0)], m_new[(2, 1)], m_new[(2, 2)]],
        ];
        let mut c = [[0f32; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    c[i][j] += b[i][k] * a[k][j];
                }
            }
        }
        self.mat = c;
        self
    }
}

impl Primitive for Rotate {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let p1 = ctx.fresh_point();
        // GLSL mat3 constructor is column-major: mat3(col0, col1, col2)
        // col i = (mat[0][i], mat[1][i], mat[2][i])
        let m = &self.mat;
        ctx.push(format!(
            "vec3 {p1} = mat3(\
            {:.8},{:.8},{:.8}, \
            {:.8},{:.8},{:.8}, \
            {:.8},{:.8},{:.8}) * {p};",
            m[0][0], m[1][0], m[2][0],
            m[0][1], m[1][1], m[2][1],
            m[0][2], m[1][2], m[2][2],
        ));
        self.inner.expression(&p1, ctx)
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        let m = &self.mat;
        let px = m[0][0] * x + m[0][1] * y + m[0][2] * z;
        let py = m[1][0] * x + m[1][1] * y + m[1][2] * z;
        let pz = m[2][0] * x + m[2][1] * y + m[2][2] * z;
        self.inner.eval([px, py, pz])
    }
    fn bbox(&self) -> Bbox {
        // Conservative: untransform the inner bbox corners and re-bound
        let b = self.inner.bbox();
        let corners = [
            [b.min[0], b.min[1], b.min[2]],
            [b.max[0], b.min[1], b.min[2]],
            [b.min[0], b.max[1], b.min[2]],
            [b.max[0], b.max[1], b.min[2]],
            [b.min[0], b.min[1], b.max[2]],
            [b.max[0], b.min[1], b.max[2]],
            [b.min[0], b.max[1], b.max[2]],
            [b.max[0], b.max[1], b.max[2]],
        ];
        // Rotate each corner by the transpose (inverse) to get world-space corners
        let m = &self.mat;
        let rotated: Vec<[f32; 3]> = corners
            .iter()
            .map(|[x, y, z]| {
                // Inverse rotation = transpose
                [
                    m[0][0] * x + m[1][0] * y + m[2][0] * z,
                    m[0][1] * x + m[1][1] * y + m[2][1] * z,
                    m[0][2] * x + m[1][2] * y + m[2][2] * z,
                ]
            })
            .collect();
        let mut min = [INF; 3];
        let mut max = [-INF; 3];
        for c in &rotated {
            for i in 0..3 {
                min[i] = min[i].min(c[i]);
                max[i] = max[i].max(c[i]);
            }
        }
        Bbox { min, max }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}

// ── Scale ─────────────────────────────────────────────────────────────────────
// Non-uniform scale: divide point components, then multiply result by min(sx,sy,sz).

#[derive(Clone)]
pub struct Scale {
    inner: Box<dyn Primitive>,
    pub s: [f32; 3],
}

impl Scale {
    pub fn new(inner: Box<dyn Primitive>, s: [f32; 3]) -> Self {
        Scale { inner, s }
    }
}

impl Primitive for Scale {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let p1 = ctx.fresh_point();
        let [sx, sy, sz] = self.s;
        ctx.push(format!("vec3 {p1} = {p} / vec3({sx:.8}, {sy:.8}, {sz:.8});"));
        let d_inner = self.inner.expression(&p1, ctx);
        let d = ctx.fresh_float();
        let min_s = sx.min(sy).min(sz);
        ctx.push(format!("float {d} = {d_inner} * {min_s:.8};"));
        d
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        let [sx, sy, sz] = self.s;
        let inner_d = self.inner.eval([x / sx, y / sy, z / sz]);
        inner_d * sx.min(sy).min(sz)
    }
    fn bbox(&self) -> Bbox {
        let b = self.inner.bbox();
        let [sx, sy, sz] = self.s;
        Bbox {
            min: [b.min[0] * sx, b.min[1] * sy, b.min[2] * sz],
            max: [b.max[0] * sx, b.max[1] * sy, b.max[2] * sz],
        }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}
