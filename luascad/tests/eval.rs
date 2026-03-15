use nalgebra as na;
use truescad_luascad::eval;
use truescad_luascad::implicit3d::Object;

// ── helpers ──────────────────────────────────────────────────────────────────

fn eval_obj(script: &str) -> Box<dyn Object<f64>> {
    let (_, obj) = eval(script).expect("eval failed");
    obj.expect("script did not call build()")
}

fn val(obj: &dyn Object<f64>, x: f64, y: f64, z: f64) -> f64 {
    obj.approx_value(&na::Point3::new(x, y, z), 0.)
}

// ── basic eval behaviour ──────────────────────────────────────────────────────

#[test]
fn eval_empty_script_returns_no_object() {
    let (output, obj) = eval("").expect("eval failed");
    assert!(obj.is_none());
    assert!(output.is_empty());
}

#[test]
fn eval_captures_print_output() {
    let (output, _) = eval(r#"print("hello world")"#).expect("eval failed");
    assert!(output.contains("hello world"), "output was: {output:?}");
}

#[test]
fn eval_captures_multiple_print_lines() {
    let (output, _) = eval("print(\"line1\")\nprint(\"line2\")").expect("eval failed");
    assert!(output.contains("line1"));
    assert!(output.contains("line2"));
}

#[test]
fn eval_syntax_error_returns_err() {
    assert!(eval("this is not valid lua !!!").is_err());
}

#[test]
fn eval_runtime_error_returns_err() {
    assert!(eval("error('boom')").is_err());
}

#[test]
fn eval_no_build_call_gives_no_object() {
    let (_, obj) = eval("local s = Sphere(1.0)").expect("eval failed");
    assert!(obj.is_none());
}

// ── primitives ────────────────────────────────────────────────────────────────

#[test]
fn eval_sphere_inside_outside() {
    let obj = eval_obj("build(Sphere(1.0))");
    // origin is inside the unit sphere → value < 0
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
    // point well outside → value > 0
    assert!(val(obj.as_ref(), 5., 0., 0.) > 0.);
}

#[test]
fn eval_sphere_bbox() {
    let obj = eval_obj("build(Sphere(2.0))");
    let bb = obj.bbox();
    assert!(bb.min.x <= -2.0);
    assert!(bb.max.x >= 2.0);
}

#[test]
fn eval_box_inside_outside() {
    let obj = eval_obj("build(Box(2, 2, 2))");
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
    assert!(val(obj.as_ref(), 5., 0., 0.) > 0.);
}

#[test]
fn eval_box_with_smoothing() {
    // Should succeed without error even with a smoothing parameter.
    eval_obj("build(Box(2, 2, 2, 0.1))");
}

#[test]
fn eval_cylinder_uniform_radius() {
    let obj = eval_obj("build(Cylinder({l=4, r=1}))");
    // centre is inside
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
    // far outside radially
    assert!(val(obj.as_ref(), 5., 0., 0.) > 0.);
    // beyond the caps
    assert!(val(obj.as_ref(), 0., 0., 5.) > 0.);
}

#[test]
fn eval_cylinder_tapered() {
    // r1 ≠ r2 exercises the Cone code path.
    let obj = eval_obj("build(Cylinder({l=4, r1=1, r2=2}))");
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
}

#[test]
fn eval_plane_x() {
    let obj = eval_obj("build(PlaneX(1.0))");
    // x=0 is inside (negative side of the plane at x=1)
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
    // x=2 is outside
    assert!(val(obj.as_ref(), 2., 0., 0.) > 0.);
}

#[test]
fn eval_plane_neg_x() {
    let obj = eval_obj("build(PlaneNegX(1.0))");
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
    assert!(val(obj.as_ref(), -2., 0., 0.) > 0.);
}

#[test]
fn eval_plane_hessian() {
    // Plane with normal pointing in +x, p=1 → same as PlaneX(1); just ensure it constructs.
    eval_obj("build(PlaneHessian({1,0,0}, 1.0))");
}

#[test]
fn eval_plane_3points() {
    // Three points defining the XY plane (z=0); just ensure it constructs.
    eval_obj("build(Plane3Points({1,0,0}, {0,1,0}, {0,0,0}))");
}

#[test]
fn eval_icylinder() {
    let obj = eval_obj("build(iCylinder(1.0))");
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
    assert!(val(obj.as_ref(), 5., 0., 0.) > 0.);
}

#[test]
fn eval_icone() {
    let obj = eval_obj("build(iCone(1.0))");
    // apex is at origin — value there should be 0 (on the surface)
    assert!(val(obj.as_ref(), 0., 0., 0.).abs() < 1e-6);
}

// ── transformations ───────────────────────────────────────────────────────────

#[test]
fn eval_translate_shifts_bbox() {
    let original = eval_obj("build(Sphere(1.0))");
    let translated = eval_obj("build(Sphere(1.0):translate(10, 0, 0))");
    // original centre inside original
    assert!(val(original.as_ref(), 0., 0., 0.) < 0.);
    // original centre now outside translated sphere
    assert!(val(translated.as_ref(), 0., 0., 0.) > 0.);
    // new centre inside translated sphere
    assert!(val(translated.as_ref(), 10., 0., 0.) < 0.);
}

#[test]
fn eval_scale() {
    let small = eval_obj("build(Sphere(1.0))");
    let big = eval_obj("build(Sphere(1.0):scale(3, 3, 3))");
    // point at radius 2: outside small, inside big
    assert!(val(small.as_ref(), 2., 0., 0.) > 0.);
    assert!(val(big.as_ref(), 2., 0., 0.) < 0.);
}

#[test]
fn eval_rotate_returns_object() {
    // Just verify rotation doesn't error and returns an object.
    eval_obj("build(Box(2,2,2):rotate(0.5, 0.5, 0))");
}

// ── boolean operations ────────────────────────────────────────────────────────

#[test]
fn eval_union_of_two_spheres() {
    let obj = eval_obj(
        "build(Union({Sphere(1.0):translate(2,0,0), Sphere(1.0):translate(-2,0,0)}))",
    );
    // both sphere centres inside the union
    assert!(val(obj.as_ref(), 2., 0., 0.) < 0.);
    assert!(val(obj.as_ref(), -2., 0., 0.) < 0.);
    // point far away is outside
    assert!(val(obj.as_ref(), 10., 0., 0.) > 0.);
}

#[test]
fn eval_intersection_of_two_planes() {
    // Intersection of PlaneX(1) and PlaneNegX(1) → slab |x| ≤ 1
    let obj = eval_obj("build(Intersection({PlaneX(1.0), PlaneNegX(1.0)}))");
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
    assert!(val(obj.as_ref(), 5., 0., 0.) > 0.);
}

#[test]
fn eval_difference() {
    // Big sphere minus a small sphere that does not reach the origin.
    let obj = eval_obj("build(Difference({Sphere(3.0), Sphere(1.0):translate(5,0,0)}))");
    // Origin is inside the big sphere and outside the subtracting sphere → inside result.
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
    // Point inside the subtracting sphere is carved out → outside result.
    assert!(val(obj.as_ref(), 5., 0., 0.) > 0.);
}

// ── deformations ──────────────────────────────────────────────────────────────

#[test]
fn eval_bend() {
    // Bender deforms the object non-trivially; just verify it builds without error.
    let obj = eval_obj("build(Bend(Box(4,1,1), 4.0))");
    assert!(obj.bbox().is_finite());
}

#[test]
fn eval_twist() {
    let obj = eval_obj("build(Twist(Box(1,1,4), 4.0))");
    assert!(val(obj.as_ref(), 0., 0., 0.) < 0.);
}

// ── sandbox security ──────────────────────────────────────────────────────────

#[test]
fn sandbox_blocks_io() {
    assert!(eval("io.open('test')").is_err());
}

#[test]
fn sandbox_blocks_os_execute() {
    assert!(eval("os.execute('echo hi')").is_err());
}

#[test]
fn sandbox_blocks_require() {
    assert!(eval("require('os')").is_err());
}

#[test]
fn sandbox_blocks_load() {
    assert!(eval("load('return 1')()").is_err());
}

#[test]
fn sandbox_blocks_dofile() {
    assert!(eval("dofile('/etc/passwd')").is_err());
}
