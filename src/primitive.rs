pub trait Primitive: Send + Sync {
    /// Generate GLSL statements into `ctx`; return the name of the float variable
    /// holding the signed distance result.
    fn expression(&self, p: &str, ctx: &mut GlslCtx) -> String;
    /// CPU evaluation for tessellation. Positive = outside, negative = inside.
    fn eval(&self, p: [f32; 3]) -> f32;
    fn bbox(&self) -> Bbox;
    fn clone_box(&self) -> Box<dyn Primitive>;
}

impl Clone for Box<dyn Primitive> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[derive(Clone, Copy)]
pub struct Bbox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bbox {
    pub fn width(&self) -> f32 {
        (self.max[0] - self.min[0])
            .max(self.max[1] - self.min[1])
            .max(self.max[2] - self.min[2])
    }

    pub fn union(&self, other: &Bbox) -> Bbox {
        Bbox {
            min: [
                self.min[0].min(other.min[0]),
                self.min[1].min(other.min[1]),
                self.min[2].min(other.min[2]),
            ],
            max: [
                self.max[0].max(other.max[0]),
                self.max[1].max(other.max[1]),
                self.max[2].max(other.max[2]),
            ],
        }
    }

    pub fn intersection(&self, other: &Bbox) -> Bbox {
        Bbox {
            min: [
                self.min[0].max(other.min[0]),
                self.min[1].max(other.min[1]),
                self.min[2].max(other.min[2]),
            ],
            max: [
                self.max[0].min(other.max[0]),
                self.max[1].min(other.max[1]),
                self.max[2].min(other.max[2]),
            ],
        }
    }
}

pub struct GlslCtx {
    counter: usize,
    pub statements: Vec<String>,
    pub helpers: Vec<String>,
}

impl GlslCtx {
    pub fn new() -> Self {
        GlslCtx {
            counter: 0,
            statements: Vec::new(),
            helpers: Vec::new(),
        }
    }

    pub fn fresh_float(&mut self) -> String {
        let n = self.counter;
        self.counter += 1;
        format!("d{n}")
    }

    pub fn fresh_point(&mut self) -> String {
        let n = self.counter;
        self.counter += 1;
        format!("p{n}")
    }

    pub fn push(&mut self, s: impl Into<String>) {
        self.statements.push(s.into());
    }

    pub fn add_helper(&mut self, src: &str) {
        if !self.helpers.iter().any(|h| h == src) {
            self.helpers.push(src.to_string());
        }
    }
}
