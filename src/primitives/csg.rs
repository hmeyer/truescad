use crate::primitive::{Bbox, GlslCtx, Primitive};

const SMIN_HELPER: &str = "\
float smin(float a, float b, float k) {
    float h = clamp(0.5 + 0.5*(b-a)/k, 0.0, 1.0);
    return mix(b, a, h) - k*h*(1.0-h);
}";

const SMAX_HELPER: &str = "\
float smax(float a, float b, float k) {
    float h = clamp(0.5 + 0.5*(b-a)/k, 0.0, 1.0);
    return mix(a, b, h) + k*h*(1.0-h);
}";

fn cpu_smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b * (1.0 - h) + a * h - k * h * (1.0 - h)
}

fn cpu_smax(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    a * (1.0 - h) + b * h + k * h * (1.0 - h)
}

/// Fold a list of GLSL variable names with `min(a, min(b, ...))`.
fn fold_min(vars: &[String]) -> String {
    match vars {
        [] => unreachable!(),
        [v] => v.clone(),
        [v, rest @ ..] => format!("min({v}, {})", fold_min(rest)),
    }
}

/// Fold a list of GLSL variable names with `max(a, max(b, ...))`.
fn fold_max(vars: &[String]) -> String {
    match vars {
        [] => unreachable!(),
        [v] => v.clone(),
        [v, rest @ ..] => format!("max({v}, {})", fold_max(rest)),
    }
}

/// Fold with `smin(a, smin(b, ..., k), k)`.
fn fold_smin(vars: &[String], k: f32) -> String {
    match vars {
        [] => unreachable!(),
        [v] => v.clone(),
        [v, rest @ ..] => format!("smin({v}, {}, {k:.8})", fold_smin(rest, k)),
    }
}

/// Fold with `smax(a, smax(b, ..., k), k)`.
fn fold_smax(vars: &[String], k: f32) -> String {
    match vars {
        [] => unreachable!(),
        [v] => v.clone(),
        [v, rest @ ..] => format!("smax({v}, {}, {k:.8})", fold_smax(rest, k)),
    }
}

// ── Union ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Union {
    pub children: Vec<Box<dyn Primitive>>,
    pub smoothing: f32,
}

impl Union {
    pub fn new(children: Vec<Box<dyn Primitive>>, smoothing: f32) -> Self {
        Union { children, smoothing }
    }
}

impl Primitive for Union {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let vars: Vec<String> = self.children.iter().map(|c| c.expression(p, ctx)).collect();
        let d = ctx.fresh_float();
        if self.smoothing > 0.0 {
            ctx.add_helper(SMIN_HELPER);
            ctx.push(format!("float {d} = {};", fold_smin(&vars, self.smoothing)));
        } else {
            ctx.push(format!("float {d} = {};", fold_min(&vars)));
        }
        d
    }
    fn eval(&self, p: [f32; 3]) -> f32 {
        let vals: Vec<f32> = self.children.iter().map(|c| c.eval(p)).collect();
        if self.smoothing > 0.0 {
            vals.into_iter().reduce(|a, b| cpu_smin(a, b, self.smoothing)).unwrap()
        } else {
            vals.into_iter().reduce(f32::min).unwrap()
        }
    }
    fn bbox(&self) -> Bbox {
        self.children.iter().map(|c| c.bbox()).reduce(|a, b| a.union(&b)).unwrap()
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}

// ── Intersection ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Intersection {
    pub children: Vec<Box<dyn Primitive>>,
    pub smoothing: f32,
}

impl Intersection {
    pub fn new(children: Vec<Box<dyn Primitive>>, smoothing: f32) -> Self {
        Intersection { children, smoothing }
    }
}

impl Primitive for Intersection {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let vars: Vec<String> = self.children.iter().map(|c| c.expression(p, ctx)).collect();
        let d = ctx.fresh_float();
        if self.smoothing > 0.0 {
            ctx.add_helper(SMAX_HELPER);
            ctx.push(format!("float {d} = {};", fold_smax(&vars, self.smoothing)));
        } else {
            ctx.push(format!("float {d} = {};", fold_max(&vars)));
        }
        d
    }
    fn eval(&self, p: [f32; 3]) -> f32 {
        let vals: Vec<f32> = self.children.iter().map(|c| c.eval(p)).collect();
        if self.smoothing > 0.0 {
            vals.into_iter().reduce(|a, b| cpu_smax(a, b, self.smoothing)).unwrap()
        } else {
            vals.into_iter().reduce(f32::max).unwrap()
        }
    }
    fn bbox(&self) -> Bbox {
        self.children.iter().map(|c| c.bbox()).reduce(|a, b| a.intersection(&b)).unwrap()
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}

// ── Difference ────────────────────────────────────────────────────────────────
// first child minus all remaining children: max(d0, max(-d1, -d2, ...))

#[derive(Clone)]
pub struct Difference {
    pub children: Vec<Box<dyn Primitive>>,
    pub smoothing: f32,
}

impl Difference {
    pub fn new(children: Vec<Box<dyn Primitive>>, smoothing: f32) -> Self {
        Difference { children, smoothing }
    }
}

impl Primitive for Difference {
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String {
        let first = self.children[0].expression(p, ctx);
        let rest_neg: Vec<String> = self.children[1..]
            .iter()
            .map(|c| {
                let v = c.expression(p, ctx);
                let neg = ctx.fresh_float();
                ctx.push(format!("float {neg} = -{v};"));
                neg
            })
            .collect();

        let mut all = vec![first];
        all.extend(rest_neg);

        let d = ctx.fresh_float();
        if self.smoothing > 0.0 {
            ctx.add_helper(SMAX_HELPER);
            ctx.push(format!("float {d} = {};", fold_smax(&all, self.smoothing)));
        } else {
            ctx.push(format!("float {d} = {};", fold_max(&all)));
        }
        d
    }
    fn eval(&self, p: [f32; 3]) -> f32 {
        let first = self.children[0].eval(p);
        let rest_neg: Vec<f32> = self.children[1..].iter().map(|c| -c.eval(p)).collect();
        let mut all = vec![first];
        all.extend(rest_neg);
        if self.smoothing > 0.0 {
            all.into_iter().reduce(|a, b| cpu_smax(a, b, self.smoothing)).unwrap()
        } else {
            all.into_iter().reduce(f32::max).unwrap()
        }
    }
    fn bbox(&self) -> Bbox {
        // Conservative: use first child's bbox
        self.children[0].bbox()
    }
    fn clone_box(&self) -> Box<dyn Primitive> {
        Box::new(self.clone())
    }
}
