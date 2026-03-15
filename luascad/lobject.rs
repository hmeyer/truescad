use super::{Float, EPSILON};
use implicit3d::{
    Bender, BoundingBox, Cone, Cylinder, Intersection, Mesh, NormalPlane, Object, PlaneNegX,
    PlaneNegY, PlaneNegZ, PlaneX, PlaneY, PlaneZ, Sphere, Twister, Union,
};
use nalgebra as na;
use piccolo::{
    Callback, CallbackReturn, Context, Error, IntoValue, Table, UserData, Value,
};
use std::cell::RefCell;
use std::rc::Rc;

pub const INFINITY: Float = 1e10;
pub const NEG_INFINITY: Float = -1e10;

/// Extract an LObject (Box<dyn Object<Float>>) from a UserData value.
fn get_object<'gc>(ud: UserData<'gc>) -> Option<Box<dyn Object<Float>>> {
    ud.downcast_static::<Option<Box<dyn Object<Float>>>>()
        .ok()
        .and_then(|o| o.clone())
}

/// Create a UserData wrapping an Object, with methods metatable attached.
fn make_lobject<'gc>(
    ctx: Context<'gc>,
    obj: Option<Box<dyn Object<Float>>>,
) -> Value<'gc> {
    let ud = UserData::new_static(&ctx, obj);
    ud.set_metatable(&ctx, Some(make_methods_metatable(ctx)));
    ud.into_value(ctx)
}

/// Create the __index metatable with translate/rotate/scale/clone methods.
fn make_methods_metatable<'gc>(ctx: Context<'gc>) -> Table<'gc> {
    let mt = Table::new(&ctx);
    let index = Table::new(&ctx);

    index
        .set(
            ctx,
            "translate",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let x: Float = stack.from_front(ctx)?;
                let y: Float = stack.from_front(ctx)?;
                let z: Float = stack.from_front(ctx)?;
                let result = get_object(ud).map(|o| o.translate(&na::Vector3::new(x, y, z)));
                stack.replace(ctx, make_lobject(ctx, result));
                Ok(CallbackReturn::Return)
            }),
        )
        .unwrap();

    index
        .set(
            ctx,
            "rotate",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let x: Float = stack.from_front(ctx)?;
                let y: Float = stack.from_front(ctx)?;
                let z: Float = stack.from_front(ctx)?;
                let result = get_object(ud).map(|o| o.rotate(&na::Vector3::new(x, y, z)));
                stack.replace(ctx, make_lobject(ctx, result));
                Ok(CallbackReturn::Return)
            }),
        )
        .unwrap();

    index
        .set(
            ctx,
            "scale",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let x: Float = stack.from_front(ctx)?;
                let y: Float = stack.from_front(ctx)?;
                let z: Float = stack.from_front(ctx)?;
                let result = get_object(ud).map(|o| o.scale(&na::Vector3::new(x, y, z)));
                stack.replace(ctx, make_lobject(ctx, result));
                Ok(CallbackReturn::Return)
            }),
        )
        .unwrap();

    index
        .set(
            ctx,
            "clone",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let result = get_object(ud);
                stack.replace(ctx, make_lobject(ctx, result));
                Ok(CallbackReturn::Return)
            }),
        )
        .unwrap();

    mt.set(ctx, "__index", index).unwrap();

    // Add __tostring metamethod
    mt.set(
        ctx,
        "__tostring",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let ud: UserData = stack.from_front(ctx)?;
            let s = match get_object(ud) {
                Some(o) => format!("LObject({:?})", o.bbox()),
                None => "LObject(nil)".to_string(),
            };
            stack.replace(ctx, s);
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    mt
}

/// Register all object constructors, CSG operations, and utility functions.
pub fn register_all<'gc>(
    ctx: Context<'gc>,
    build_result: Rc<RefCell<Option<Box<dyn Object<Float>>>>>,
    print_buffer: Rc<RefCell<String>>,
) -> Result<(), piccolo::InvalidTableKey> {
    // === build() function ===
    ctx.set_global(
        "build",
        Callback::from_fn(&ctx, move |_ctx, _, mut stack| {
            let ud = stack.pop_front();
            if let Value::UserData(ud) = ud {
                if let Some(obj) = get_object(ud) {
                    *build_result.borrow_mut() = Some(obj);
                }
            }
            stack.clear();
            Ok(CallbackReturn::Return)
        }),
    )?;

    // === print() function ===
    ctx.set_global(
        "print",
        Callback::from_fn(&ctx, move |_ctx, _, mut stack| {
            let mut buf = print_buffer.borrow_mut();
            let len = stack.len();
            for i in 0..len {
                let val = stack.get(i);
                match val {
                    Value::Nil => buf.push_str("nil"),
                    Value::Boolean(b) => buf.push_str(&b.to_string()),
                    Value::Integer(n) => buf.push_str(&n.to_string()),
                    Value::Number(n) => buf.push_str(&n.to_string()),
                    Value::String(s) => {
                        if let Ok(s) = s.to_str() {
                            buf.push_str(s);
                        } else {
                            buf.push_str("<non-utf8>");
                        }
                    }
                    _ => buf.push_str(&format!("{:?}", val)),
                }
                buf.push('\t');
            }
            buf.push('\n');
            stack.clear();
            Ok(CallbackReturn::Return)
        }),
    )?;

    // === Primitive constructors ===
    ctx.set_global(
        "Sphere",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let radius: Float = stack.consume(ctx)?;
            stack.replace(ctx, make_lobject(ctx, Some(Box::new(Sphere::new(radius)))));
            Ok(CallbackReturn::Return)
        }),
    )?;

    ctx.set_global(
        "iCylinder",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let radius: Float = stack.consume(ctx)?;
            stack.replace(
                ctx,
                make_lobject(ctx, Some(Box::new(Cylinder::new(radius)))),
            );
            Ok(CallbackReturn::Return)
        }),
    )?;

    ctx.set_global(
        "iCone",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let slope: Float = stack.consume(ctx)?;
            stack.replace(
                ctx,
                make_lobject(ctx, Some(Box::new(Cone::new(slope, 0.)))),
            );
            Ok(CallbackReturn::Return)
        }),
    )?;

    // Plane constructors
    macro_rules! register_plane {
        ($ctx:expr, $name:expr, $type:ident) => {
            $ctx.set_global(
                $name,
                Callback::from_fn(&$ctx, |ctx, _, mut stack| {
                    let d: Float = stack.consume(ctx)?;
                    stack.replace(ctx, make_lobject(ctx, Some(Box::new($type::new(d)))));
                    Ok(CallbackReturn::Return)
                }),
            )?;
        };
    }

    register_plane!(ctx, "PlaneX", PlaneX);
    register_plane!(ctx, "PlaneY", PlaneY);
    register_plane!(ctx, "PlaneZ", PlaneZ);
    register_plane!(ctx, "PlaneNegX", PlaneNegX);
    register_plane!(ctx, "PlaneNegY", PlaneNegY);
    register_plane!(ctx, "PlaneNegZ", PlaneNegZ);

    ctx.set_global(
        "Bend",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let ud: UserData = stack.from_front(ctx)?;
            let width: Float = stack.from_front(ctx)?;
            let result = get_object(ud).map(|obj| {
                Box::new(Bender::new(obj, width)) as Box<dyn Object<Float>>
            });
            stack.replace(ctx, make_lobject(ctx, result));
            Ok(CallbackReturn::Return)
        }),
    )?;

    ctx.set_global(
        "Twist",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let ud: UserData = stack.from_front(ctx)?;
            let height: Float = stack.from_front(ctx)?;
            let result = get_object(ud).map(|obj| {
                Box::new(Twister::new(obj, height)) as Box<dyn Object<Float>>
            });
            stack.replace(ctx, make_lobject(ctx, result));
            Ok(CallbackReturn::Return)
        }),
    )?;

    ctx.set_global(
        "Mesh",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let filename: std::string::String = stack.consume(ctx)?;
            let result = match Mesh::try_new(&filename) {
                Ok(mesh) => Some(Box::new(mesh) as Box<dyn Object<Float>>),
                Err(_) => None,
            };
            stack.replace(ctx, make_lobject(ctx, result));
            Ok(CallbackReturn::Return)
        }),
    )?;

    // === CSG operations ===
    // Extract objects from a Lua table directly in Rust callbacks.
    fn extract_objects<'gc>(
        ctx: Context<'gc>,
        table: Table<'gc>,
    ) -> Vec<Box<dyn Object<Float>>> {
        let mut objects = Vec::new();
        let len = table.length();
        for i in 1..=len {
            let val = table.get(ctx, i);
            if let Value::UserData(ud) = val {
                if let Some(obj) = get_object(ud) {
                    objects.push(obj);
                }
            }
        }
        objects
    }

    ctx.set_global(
        "Union",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let table: Table = stack.from_front(ctx)?;
            let smooth: Float = match stack.from_front::<Value>(ctx)? {
                Value::Nil => 0.0,
                Value::Number(n) => n,
                Value::Integer(n) => n as Float,
                _ => 0.0,
            };
            let objects = extract_objects(ctx, table);
            let result = if objects.is_empty() {
                None
            } else {
                Union::from_vec(objects, smooth).map(|o| o as Box<dyn Object<Float>>)
            };
            stack.replace(ctx, make_lobject(ctx, result));
            Ok(CallbackReturn::Return)
        }),
    )?;

    ctx.set_global(
        "Intersection",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let table: Table = stack.from_front(ctx)?;
            let smooth: Float = match stack.from_front::<Value>(ctx)? {
                Value::Nil => 0.0,
                Value::Number(n) => n,
                Value::Integer(n) => n as Float,
                _ => 0.0,
            };
            let objects = extract_objects(ctx, table);
            let result = if objects.is_empty() {
                None
            } else {
                Intersection::from_vec(objects, smooth)
            };
            stack.replace(ctx, make_lobject(ctx, result));
            Ok(CallbackReturn::Return)
        }),
    )?;

    ctx.set_global(
        "Difference",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let table: Table = stack.from_front(ctx)?;
            let smooth: Float = match stack.from_front::<Value>(ctx)? {
                Value::Nil => 0.0,
                Value::Number(n) => n,
                Value::Integer(n) => n as Float,
                _ => 0.0,
            };
            let objects = extract_objects(ctx, table);
            let result = if objects.is_empty() {
                None
            } else {
                Intersection::difference_from_vec(objects, smooth)
            };
            stack.replace(ctx, make_lobject(ctx, result));
            Ok(CallbackReturn::Return)
        }),
    )?;

    // === User-facing wrapper functions ===

    // Box(x, y, z, smooth)
    ctx.set_global("Box", Callback::from_fn(&ctx, |ctx, _, mut stack| {
        let x: Float = stack.from_front(ctx).map_err(|_| -> Error {
            "Box: all arguments must be numbers".into_value(ctx).into()
        })?;
        let y: Float = stack.from_front(ctx).map_err(|_| -> Error {
            "Box: all arguments must be numbers".into_value(ctx).into()
        })?;
        let z: Float = stack.from_front(ctx).map_err(|_| -> Error {
            "Box: all arguments must be numbers".into_value(ctx).into()
        })?;
        let smooth: Float = match stack.from_front::<Value>(ctx)? {
            Value::Nil => 0.0,
            Value::Number(n) => n,
            Value::Integer(n) => n as Float,
            _ => 0.0,
        };
        // Build the box directly
        let obj = Intersection::from_vec(
            vec![
                Box::new(PlaneX::new(x / 2.0)),
                Box::new(PlaneY::new(y / 2.0)),
                Box::new(PlaneZ::new(z / 2.0)),
                Box::new(PlaneNegX::new(x / 2.0)),
                Box::new(PlaneNegY::new(y / 2.0)),
                Box::new(PlaneNegZ::new(z / 2.0)),
            ],
            smooth,
        )
        .unwrap();
        stack.replace(ctx, make_lobject(ctx, Some(obj)));
        Ok(CallbackReturn::Return)
    }))?;

    // Cylinder({l=, r= or r1=, r2=, s=}) - takes a table argument
    ctx.set_global(
        "Cylinder",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let table: Table = stack.from_front(ctx).map_err(|_| -> Error {
                "Cylinder: argument must be a table".into_value(ctx).into()
            })?;

            let l = match table.get(ctx, ctx.intern(b"l")) {
                Value::Number(n) => n,
                Value::Integer(n) => n as Float,
                _ => {
                    return Err("l must be a valid number".into_value(ctx).into());
                }
            };

            let (r1, r2) = match table.get(ctx, ctx.intern(b"r")) {
                Value::Number(n) => (n, n),
                Value::Integer(n) => (n as Float, n as Float),
                _ => {
                    let r1_val = table.get(ctx, ctx.intern(b"r1"));
                    let r2_val = table.get(ctx, ctx.intern(b"r2"));
                    match (r1_val, r2_val) {
                        (Value::Number(r1), Value::Number(r2)) => (r1, r2),
                        (Value::Integer(r1), Value::Number(r2)) => (r1 as Float, r2),
                        (Value::Number(r1), Value::Integer(r2)) => (r1, r2 as Float),
                        (Value::Integer(r1), Value::Integer(r2)) => (r1 as Float, r2 as Float),
                        _ => {
                            return Err(
                                "specify either r or r1 and r2".into_value(ctx).into()
                            );
                        }
                    }
                }
            };

            let smooth = match table.get(ctx, ctx.intern(b"s")) {
                Value::Number(n) => n,
                Value::Integer(n) => n as Float,
                _ => 0.0,
            };

            // Build the cylinder
            let mut conie: Box<dyn Object<Float>>;
            if (r1 - r2).abs() < EPSILON {
                conie = Box::new(Cylinder::new(r1));
            } else {
                let slope = (r2 - r1).abs() / l;
                let offset = if r1 < r2 {
                    -r1 / slope - l * 0.5
                } else {
                    r2 / slope + l * 0.5
                };
                conie = Box::new(Cone::new(slope, offset));
                let rmax = r1.max(r2);
                let conie_box = BoundingBox::new(
                    &na::Point3::new(-rmax, -rmax, NEG_INFINITY),
                    &na::Point3::new(rmax, rmax, INFINITY),
                );
                conie.set_bbox(&conie_box);
            }
            let obj = Intersection::from_vec(
                vec![
                    conie,
                    Box::new(PlaneZ::new(l / 2.0)),
                    Box::new(PlaneNegZ::new(l / 2.0)),
                ],
                smooth,
            )
            .unwrap();
            stack.replace(ctx, make_lobject(ctx, Some(obj)));
            Ok(CallbackReturn::Return)
        }),
    )?;

    // PlaneHessian(n, p) - n is a table of 3 numbers, p is a number
    ctx.set_global(
        "PlaneHessian",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let n_table: Table = stack.from_front(ctx).map_err(|_| -> Error {
                "PlaneHessian: first argument (normal) must be a table of 3 numbers"
                    .into_value(ctx)
                    .into()
            })?;
            let p: Float = stack.from_front(ctx).map_err(|_| -> Error {
                "PlaneHessian: second argument must be a number".into_value(ctx).into()
            })?;

            let nx = to_float(n_table.get(ctx, 1i64));
            let ny = to_float(n_table.get(ctx, 2i64));
            let nz = to_float(n_table.get(ctx, 3i64));

            match (nx, ny, nz) {
                (Some(nx), Some(ny), Some(nz)) => {
                    let obj = NormalPlane::from_normal_and_p(na::Vector3::new(nx, ny, nz), p);
                    stack.replace(ctx, make_lobject(ctx, Some(Box::new(obj))));
                    Ok(CallbackReturn::Return)
                }
                _ => Err("PlaneHessian: normal must contain 3 numbers"
                    .into_value(ctx)
                    .into()),
            }
        }),
    )?;

    // Plane3Points(a, b, c) - each is a table of 3 numbers
    ctx.set_global(
        "Plane3Points",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let a_table: Table = stack.from_front(ctx).map_err(|_| -> Error {
                "Plane3Points: arguments must be tables of 3 numbers"
                    .into_value(ctx)
                    .into()
            })?;
            let b_table: Table = stack.from_front(ctx).map_err(|_| -> Error {
                "Plane3Points: arguments must be tables of 3 numbers"
                    .into_value(ctx)
                    .into()
            })?;
            let c_table: Table = stack.from_front(ctx).map_err(|_| -> Error {
                "Plane3Points: arguments must be tables of 3 numbers"
                    .into_value(ctx)
                    .into()
            })?;

            let a = table_to_point3(ctx, a_table);
            let b = table_to_point3(ctx, b_table);
            let c = table_to_point3(ctx, c_table);

            match (a, b, c) {
                (Some(a), Some(b), Some(c)) => {
                    let obj = NormalPlane::from_3_points(&a, &b, &c);
                    stack.replace(ctx, make_lobject(ctx, Some(Box::new(obj))));
                    Ok(CallbackReturn::Return)
                }
                _ => Err("Plane3Points: all elements must be numbers"
                    .into_value(ctx)
                    .into()),
            }
        }),
    )?;

    Ok(())
}

fn to_float(val: Value) -> Option<Float> {
    match val {
        Value::Number(n) => Some(n),
        Value::Integer(n) => Some(n as Float),
        _ => None,
    }
}

fn table_to_point3<'gc>(ctx: Context<'gc>, table: Table<'gc>) -> Option<na::Point3<Float>> {
    let x = to_float(table.get(ctx, 1i64))?;
    let y = to_float(table.get(ctx, 2i64))?;
    let z = to_float(table.get(ctx, 3i64))?;
    Some(na::Point3::new(x, y, z))
}

