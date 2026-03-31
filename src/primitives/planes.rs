use crate::primitive::{Bbox, GlslCtx, Primitive};

const INF: f32 = 1e10;

// ── Axis-aligned half-space planes ───────────────────────────────────────────
// Convention (matching implicit3d):
//   PlaneX(d)    — inside when x ≤ d,  SDF = p.x - d
//   PlaneNegX(d) — inside when x ≥ -d, SDF = -p.x - d
//   (and similarly for Y, Z)

macro_rules! axis_plane {
    (
        $name:ident,
        $glsl_expr:literal,       // e.g. "{p}.x"  or  "-{p}.x"
        $eval_fn:expr,            // closure |x,y,z| -> f32 (before subtracting d)
        $bbox_min:expr,           // closure |d| -> [f32;3]
        $bbox_max:expr            // closure |d| -> [f32;3]
    ) => {
        #[derive(Clone)]
        pub struct $name {
            pub d: f32,
        }

        impl $name {
            pub fn new(d: f32) -> Self {
                $name { d }
            }
        }

        impl Primitive for $name {
            fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
                let v = ctx.fresh_float();
                let expr = format!($glsl_expr, p = p);
                ctx.push(format!("float {v} = {expr} - {:.8};", self.d));
                v
            }
            fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
                ($eval_fn)(x, y, z) - self.d
            }
            fn bbox(&self) -> Bbox {
                Bbox {
                    min: ($bbox_min)(self.d),
                    max: ($bbox_max)(self.d),
                }
            }
            fn clone_box(&self) -> Box<dyn Primitive> {
                Box::new(self.clone())
            }
        }
    };
}

axis_plane!(PlaneX,    "{p}.x",  |x: f32, _y: f32, _z: f32| x,  |_d: f32| [-INF, -INF, -INF], |d: f32| [d, INF, INF]);
axis_plane!(PlaneNegX, "-{p}.x", |x: f32, _y: f32, _z: f32| -x, |d: f32| [-d, -INF, -INF],    |_d: f32| [INF, INF, INF]);
axis_plane!(PlaneY,    "{p}.y",  |_x: f32, y: f32, _z: f32| y,  |_d: f32| [-INF, -INF, -INF], |d: f32| [INF, d, INF]);
axis_plane!(PlaneNegY, "-{p}.y", |_x: f32, y: f32, _z: f32| -y, |d: f32| [-INF, -d, -INF],    |_d: f32| [INF, INF, INF]);
axis_plane!(PlaneZ,    "{p}.z",  |_x: f32, _y: f32, z: f32| z,  |_d: f32| [-INF, -INF, -INF], |d: f32| [INF, INF, d]);
axis_plane!(PlaneNegZ, "-{p}.z", |_x: f32, _y: f32, z: f32| -z, |d: f32| [-INF, -INF, -d],    |_d: f32| [INF, INF, INF]);

// ── Arbitrary normal plane ────────────────────────────────────────────────────
// Hessian form: SDF = dot(normal, p) - p_offset

#[derive(Clone)]
pub struct NormalPlane {
    pub normal: [f32; 3],
    pub p_offset: f32,
}

impl NormalPlane {
    pub fn from_normal_and_p(normal: [f32; 3], p_offset: f32) -> Self {
        NormalPlane { normal, p_offset }
    }

    pub fn from_3_points(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Self {
        let v1 = [a[0] - c[0], a[1] - c[1], a[2] - c[2]];
        let v2 = [b[0] - c[0], b[1] - c[1], b[2] - c[2]];
        let nx = v1[1] * v2[2] - v1[2] * v2[1];
        let ny = v1[2] * v2[0] - v1[0] * v2[2];
        let nz = v1[0] * v2[1] - v1[1] * v2[0];
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        let normal = [nx / len, ny / len, nz / len];
        let p_offset = normal[0] * a[0] + normal[1] * a[1] + normal[2] * a[2];
        NormalPlane { normal, p_offset }
    }
}

impl Primitive for NormalPlane {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let d = ctx.fresh_float();
        let [nx, ny, nz] = self.normal;
        ctx.push(format!(
            "float {d} = dot({p}, vec3({nx:.8}, {ny:.8}, {nz:.8})) - {:.8};",
            self.p_offset
        ));
        d
    }
    fn eval(&self, [x, y, z]: [f32; 3]) -> f32 {
        self.normal[0] * x + self.normal[1] * y + self.normal[2] * z - self.p_offset
    }
    fn bbox(&self) -> Bbox {
        Bbox { min: [-INF, -INF, -INF], max: [INF, INF, INF] }
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}
