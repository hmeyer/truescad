use crate::eval;
use crate::implicit3d::Object;

fn eval_ok(script: &str) -> (String, Option<Box<dyn Object<f64>>>) {
    eval(script).expect(&format!("eval failed for script: {}", script))
}

fn eval_object(script: &str) -> Box<dyn Object<f64>> {
    let (_, obj) = eval_ok(script);
    obj.expect(&format!("no object returned for script: {}", script))
}

fn assert_bbox_approx(obj: &dyn Object<f64>, expected_min: [f64; 3], expected_max: [f64; 3]) {
    let bb = obj.bbox();
    let tolerance = 0.1;
    assert!(
        (bb.min.x - expected_min[0]).abs() < tolerance
            && (bb.min.y - expected_min[1]).abs() < tolerance
            && (bb.min.z - expected_min[2]).abs() < tolerance
            && (bb.max.x - expected_max[0]).abs() < tolerance
            && (bb.max.y - expected_max[1]).abs() < tolerance
            && (bb.max.z - expected_max[2]).abs() < tolerance,
        "bbox mismatch: min=({},{},{}) max=({},{},{}) expected min={:?} max={:?}",
        bb.min.x,
        bb.min.y,
        bb.min.z,
        bb.max.x,
        bb.max.y,
        bb.max.z,
        expected_min,
        expected_max
    );
}

// === Primitive constructors ===

#[test]
fn test_sphere() {
    let obj = eval_object("build(Sphere(1.0))");
    assert_bbox_approx(&*obj, [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
}

#[test]
fn test_sphere_different_radius() {
    let obj = eval_object("build(Sphere(2.5))");
    assert_bbox_approx(&*obj, [-2.5, -2.5, -2.5], [2.5, 2.5, 2.5]);
}

#[test]
fn test_box() {
    let obj = eval_object("build(Box(2, 4, 6, 0))");
    assert_bbox_approx(&*obj, [-1.0, -2.0, -3.0], [1.0, 2.0, 3.0]);
}

#[test]
fn test_box_with_smooth() {
    // Smooth rounding shouldn't change the bounding box significantly
    let obj = eval_object("build(Box(2, 2, 2, 0.3))");
    let bb = obj.bbox();
    // With smoothing the bbox is still approximately the same
    assert!(bb.max.x > 0.5 && bb.max.x <= 1.1);
    assert!(bb.max.y > 0.5 && bb.max.y <= 1.1);
    assert!(bb.max.z > 0.5 && bb.max.z <= 1.1);
}

#[test]
fn test_cylinder_uniform_radius() {
    let obj = eval_object("build(Cylinder{l=4, r=1})");
    assert_bbox_approx(&*obj, [-1.0, -1.0, -2.0], [1.0, 1.0, 2.0]);
}

#[test]
fn test_cylinder_two_radii() {
    let obj = eval_object("build(Cylinder{l=4, r1=1, r2=2, s=0})");
    let bb = obj.bbox();
    assert!((bb.max.z - 2.0).abs() < 0.1);
    assert!((bb.min.z - -2.0).abs() < 0.1);
    // Max radius is 2
    assert!(bb.max.x >= 1.9 && bb.max.x <= 2.1);
}

#[test]
fn test_icylinder() {
    let obj = eval_object("build(iCylinder(1.5))");
    let bb = obj.bbox();
    assert!((bb.max.x - 1.5).abs() < 0.1);
    assert!((bb.max.y - 1.5).abs() < 0.1);
}

#[test]
fn test_icone() {
    let obj = eval_object("build(iCone(1.0))");
    let _bb = obj.bbox(); // Just verify it creates successfully
}

#[test]
fn test_plane_x() {
    let obj = eval_object("build(PlaneX(2.0))");
    let bb = obj.bbox();
    assert!((bb.max.x - 2.0).abs() < 0.1);
}

#[test]
fn test_plane_neg_x() {
    let obj = eval_object("build(PlaneNegX(2.0))");
    let bb = obj.bbox();
    assert!((bb.min.x - -2.0).abs() < 0.1);
}

#[test]
fn test_plane_y() {
    let obj = eval_object("build(PlaneY(3.0))");
    let bb = obj.bbox();
    assert!((bb.max.y - 3.0).abs() < 0.1);
}

#[test]
fn test_plane_neg_y() {
    let obj = eval_object("build(PlaneNegY(3.0))");
    let bb = obj.bbox();
    assert!((bb.min.y - -3.0).abs() < 0.1);
}

#[test]
fn test_plane_z() {
    let obj = eval_object("build(PlaneZ(1.0))");
    let bb = obj.bbox();
    assert!((bb.max.z - 1.0).abs() < 0.1);
}

#[test]
fn test_plane_neg_z() {
    let obj = eval_object("build(PlaneNegZ(1.0))");
    let bb = obj.bbox();
    assert!((bb.min.z - -1.0).abs() < 0.1);
}

#[test]
fn test_plane_hessian() {
    let obj = eval_object("build(PlaneHessian({0, 0, 1}, 2.0))");
    let _bb = obj.bbox();
}

#[test]
fn test_plane_3points() {
    let obj = eval_object("build(Plane3Points({1,0,0}, {0,1,0}, {0,0,1}))");
    let _bb = obj.bbox();
}

// === CSG operations ===

#[test]
fn test_union() {
    let obj = eval_object(
        "s1 = Sphere(1.0):translate(2, 0, 0)
         s2 = Sphere(1.0):translate(-2, 0, 0)
         build(Union({s1, s2}))",
    );
    let bb = obj.bbox();
    assert!((bb.min.x - -3.0).abs() < 0.1);
    assert!((bb.max.x - 3.0).abs() < 0.1);
}

#[test]
fn test_union_smooth() {
    let obj = eval_object(
        "s1 = Sphere(1.0):translate(1, 0, 0)
         s2 = Sphere(1.0):translate(-1, 0, 0)
         build(Union({s1, s2}, 0.5))",
    );
    let _bb = obj.bbox();
}

#[test]
fn test_intersection() {
    let obj = eval_object(
        "s1 = Sphere(2.0)
         b1 = Box(1, 1, 1, 0)
         build(Intersection({s1, b1}))",
    );
    let bb = obj.bbox();
    // Intersection should be bounded by the box
    assert!(bb.max.x <= 0.6);
    assert!(bb.max.y <= 0.6);
    assert!(bb.max.z <= 0.6);
}

#[test]
fn test_difference() {
    let obj = eval_object(
        "cube = Box(2, 2, 2, 0)
         sphere = Sphere(0.5)
         build(Difference({cube, sphere}, 0))",
    );
    let bb = obj.bbox();
    // Outer bbox should still be roughly the box
    assert!((bb.max.x - 1.0).abs() < 0.1);
}

// === Object methods ===

#[test]
fn test_translate() {
    let obj = eval_object("build(Sphere(1.0):translate(5, 0, 0))");
    let bb = obj.bbox();
    assert!((bb.min.x - 4.0).abs() < 0.1);
    assert!((bb.max.x - 6.0).abs() < 0.1);
}

#[test]
fn test_translate_3d() {
    let obj = eval_object("build(Sphere(1.0):translate(1, 2, 3))");
    let bb = obj.bbox();
    assert!((bb.min.x - 0.0).abs() < 0.1);
    assert!((bb.min.y - 1.0).abs() < 0.1);
    assert!((bb.min.z - 2.0).abs() < 0.1);
    assert!((bb.max.x - 2.0).abs() < 0.1);
    assert!((bb.max.y - 3.0).abs() < 0.1);
    assert!((bb.max.z - 4.0).abs() < 0.1);
}

#[test]
fn test_scale() {
    let obj = eval_object("build(Sphere(1.0):scale(2, 3, 4))");
    let bb = obj.bbox();
    assert!((bb.max.x - 2.0).abs() < 0.1);
    assert!((bb.max.y - 3.0).abs() < 0.1);
    assert!((bb.max.z - 4.0).abs() < 0.1);
}

#[test]
fn test_rotate() {
    // Rotating a box should change the bbox (AABB recalculation)
    let obj = eval_object("build(Box(2, 4, 2, 0):rotate(0, 0, 0))");
    let bb = obj.bbox();
    // With no rotation, bbox should be unchanged
    assert!((bb.max.x - 1.0).abs() < 0.2);
    assert!((bb.max.y - 2.0).abs() < 0.2);
}

#[test]
fn test_clone() {
    let obj = eval_object(
        "s = Sphere(1.0)
         c = s:clone()
         build(Union({s:translate(2, 0, 0), c:translate(-2, 0, 0)}))",
    );
    let bb = obj.bbox();
    assert!((bb.min.x - -3.0).abs() < 0.1);
    assert!((bb.max.x - 3.0).abs() < 0.1);
}

#[test]
fn test_chained_transforms() {
    let obj = eval_object("build(Sphere(1.0):translate(5, 0, 0):scale(2, 2, 2))");
    let bb = obj.bbox();
    assert!((bb.min.x - 8.0).abs() < 0.2);
    assert!((bb.max.x - 12.0).abs() < 0.2);
}

// === Print / output ===

#[test]
fn test_print_output() {
    let (output, _) = eval_ok("print('hello world')");
    assert!(output.contains("hello"), "print output: {}", output);
    assert!(output.contains("world"), "print output: {}", output);
}

#[test]
fn test_print_multiple_args() {
    let (output, _) = eval_ok("print('a', 'b', 'c')");
    assert!(output.contains("a"), "print output: {}", output);
    assert!(output.contains("b"), "print output: {}", output);
    assert!(output.contains("c"), "print output: {}", output);
}

#[test]
fn test_print_numbers() {
    let (output, _) = eval_ok("print(42)");
    assert!(output.contains("42"), "print output: {}", output);
}

// === No build call ===

#[test]
fn test_no_build_returns_none() {
    let (_, obj) = eval_ok("x = Sphere(1.0)");
    assert!(obj.is_none());
}

// === Error handling ===

#[test]
fn test_syntax_error() {
    let result = eval("this is not valid lua !@#$");
    assert!(result.is_err());
}

#[test]
fn test_runtime_error() {
    let result = eval("error('boom')");
    assert!(result.is_err());
}

#[test]
fn test_box_type_error() {
    let result = eval("Box('a', 'b', 'c', 0)");
    assert!(result.is_err());
}

#[test]
fn test_cylinder_missing_args() {
    let result = eval("Cylinder{}");
    assert!(result.is_err());
}

#[test]
fn test_cylinder_missing_radius() {
    let result = eval("Cylinder{l=4}");
    assert!(result.is_err());
}

// === Sandbox ===

#[test]
fn test_sandbox_blocks_io() {
    let result = eval("io.open('/etc/passwd')");
    assert!(result.is_err());
}

#[test]
fn test_sandbox_blocks_os_execute() {
    let result = eval("os.execute('ls')");
    assert!(result.is_err());
}

#[test]
fn test_sandbox_allows_math() {
    let (output, _) = eval_ok("print(math.sqrt(4))");
    assert!(output.contains("2"), "math.sqrt output: {}", output);
}

#[test]
fn test_sandbox_allows_string_ops() {
    let (output, _) = eval_ok("print(string.upper('hello'))");
    assert!(output.contains("HELLO"), "string.upper output: {}", output);
}

#[test]
fn test_sandbox_allows_table_ops() {
    // piccolo's table stdlib has pack/unpack only (no sort/insert)
    let (output, _) = eval_ok(
        "t = table.pack(10, 20, 30)
         print(t[1], t[2], t[3])",
    );
    assert!(output.contains("10"), "table ops output: {}", output);
    assert!(output.contains("20"), "table ops output: {}", output);
    assert!(output.contains("30"), "table ops output: {}", output);
}

// === Complex scripts ===

#[test]
fn test_xplicit_example() {
    // This is the example script from xplicit.lua
    let obj = eval_object(
        "cube = Box(1,1,1,0.3)
         sphere = Sphere(0.5)
         diff = Difference({cube, sphere}, 0.3)
         diff = diff:scale(15,15,15)
         build(diff)",
    );
    let bb = obj.bbox();
    // Scaled by 15, original box is 1x1x1
    assert!(bb.max.x > 5.0);
    assert!(bb.max.x < 10.0);
}

#[test]
fn test_multiple_unions() {
    let obj = eval_object(
        "s1 = Sphere(1.0):translate(-3, 0, 0)
         s2 = Sphere(1.0)
         s3 = Sphere(1.0):translate(3, 0, 0)
         build(Union({s1, s2, s3}))",
    );
    let bb = obj.bbox();
    assert!((bb.min.x - -4.0).abs() < 0.1);
    assert!((bb.max.x - 4.0).abs() < 0.1);
}

#[test]
fn test_nested_csg() {
    let obj = eval_object(
        "a = Union({Sphere(1.0), Box(1, 1, 1, 0)})
         b = Sphere(0.3)
         build(Difference({a, b}, 0))",
    );
    let _bb = obj.bbox();
}

#[test]
fn test_lua_loop() {
    let obj = eval_object(
        "objects = {}
         for i = 1, 3 do
           objects[i] = Sphere(0.5):translate(i * 2, 0, 0)
         end
         build(Union(objects))",
    );
    let bb = obj.bbox();
    assert!(bb.max.x > 6.0);
}

#[test]
fn test_lua_function_definition() {
    let obj = eval_object(
        "function make_ball(x, y, z, r)
           return Sphere(r):translate(x, y, z)
         end
         build(make_ball(1, 2, 3, 0.5))",
    );
    let bb = obj.bbox();
    assert!((bb.min.x - 0.5).abs() < 0.1);
}

#[test]
fn test_bend() {
    let obj = eval_object(
        "b = Box(4, 1, 1, 0)
         build(Bend(b, 2.0))",
    );
    let _bb = obj.bbox();
}

#[test]
fn test_twist() {
    let obj = eval_object(
        "b = Box(1, 1, 4, 0)
         build(Twist(b, 2.0))",
    );
    let _bb = obj.bbox();
}

// === Approx value / normal (implicit surface evaluation) ===

#[test]
fn test_sphere_approx_value() {
    let obj = eval_object("build(Sphere(1.0))");
    use nalgebra as na;
    // implicit3d uses negative-inside convention:
    // Center of sphere should have negative value (inside)
    let center_val = obj.approx_value(&na::Point3::new(0.0, 0.0, 0.0), 0.1);
    assert!(center_val < 0.0, "center value: {}", center_val);
    // Point well outside sphere should have positive value
    let outside_val = obj.approx_value(&na::Point3::new(2.0, 0.0, 0.0), 0.1);
    assert!(outside_val > 0.0, "outside value: {}", outside_val);
}

#[test]
fn test_sphere_normal() {
    let obj = eval_object("build(Sphere(1.0))");
    use nalgebra as na;
    // Normal at +x surface should point in +x direction
    let n = obj.normal(&na::Point3::new(1.0, 0.0, 0.0));
    assert!(n.x > 0.9, "normal x: {}", n.x);
    assert!(n.y.abs() < 0.1, "normal y: {}", n.y);
    assert!(n.z.abs() < 0.1, "normal z: {}", n.z);
}
