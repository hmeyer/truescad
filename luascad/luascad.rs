use std::sync::{Arc, Mutex};

use piccolo::{
    Callback, CallbackReturn, Closure, Context, Executor, IntoValue, Lua, MetaMethod, Table,
    UserData, Value,
};

use implicit3d::{
    BoundingBox, Bender, Cone, Cylinder, Intersection, Mesh, NormalPlane, Object, PlaneNegX,
    PlaneNegY, PlaneNegZ, PlaneX, PlaneY, PlaneZ, Sphere, Twister, Union,
};
use nalgebra as na;

use crate::{Float, EPSILON};

pub type EvalResult = Result<(String, Option<Box<dyn Object<Float>>>), piccolo::StaticError>;

/// Lua-visible wrapper around an implicit3d object.
pub struct LObject(pub Option<Box<dyn Object<Float>>>);

impl LObject {
    fn as_object(&self) -> Option<Box<dyn Object<Float>>> {
        self.0.as_ref().map(|o| o.clone_box())
    }
}

// ── internal helpers ──────────────────────────────────────────────────────────

/// Wrap an LObject as piccolo static UserData and attach the shared methods metatable.
fn wrap_object(ctx: Context<'_>, obj: LObject) -> Value<'_> {
    let ud = UserData::new_static(&ctx, obj);
    if let Value::Table(mt) = ctx.get_global("__lobj_mt") {
        ud.set_metatable(&ctx, Some(mt));
    }
    ud.into()
}

/// Read an integer-keyed Lua array table and collect implicit3d objects out of it.
fn objects_from_table<'gc>(
    ctx: Context<'gc>,
    table: Table<'gc>,
) -> Result<Vec<Box<dyn Object<Float>>>, piccolo::Error<'gc>> {
    let mut objects = Vec::new();
    let len = table.length() as usize;
    for i in 1..=len {
        let val = table.get(ctx, i as i64);
        match val {
            Value::UserData(ud) => {
                let obj = ud
                    .downcast_static::<LObject>()
                    .map_err(|_| "expected LObject in list".into_value(ctx))?;
                match &obj.0 {
                    Some(o) => objects.push(o.clone_box()),
                    None => {
                        return Err("nil object in list".into_value(ctx).into());
                    }
                }
            }
            _ => return Err("expected LObject in list".into_value(ctx).into()),
        }
    }
    Ok(objects)
}

// ── setup functions ───────────────────────────────────────────────────────────

/// Register a shared `__lobj_mt` metatable that provides :translate/:rotate/:scale/:clone.
fn setup_methods_metatable(ctx: Context<'_>) {
    let methods = Table::new(&ctx);

    methods
        .set(
            ctx,
            "translate",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let (x, y, z): (Float, Float, Float) = stack.consume(ctx)?;
                let obj = ud.downcast_static::<LObject>()?;
                let new_obj = LObject(obj.0.as_ref().map(|o| o.clone_box().translate(&na::Vector3::new(x, y, z))));
                stack.replace(ctx, wrap_object(ctx, new_obj));
                Ok(CallbackReturn::Return)
            }),
        )
        .unwrap();

    methods
        .set(
            ctx,
            "rotate",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let (x, y, z): (Float, Float, Float) = stack.consume(ctx)?;
                let obj = ud.downcast_static::<LObject>()?;
                let new_obj = LObject(obj.0.as_ref().map(|o| o.clone_box().rotate(&na::Vector3::new(x, y, z))));
                stack.replace(ctx, wrap_object(ctx, new_obj));
                Ok(CallbackReturn::Return)
            }),
        )
        .unwrap();

    methods
        .set(
            ctx,
            "scale",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let (x, y, z): (Float, Float, Float) = stack.consume(ctx)?;
                let obj = ud.downcast_static::<LObject>()?;
                let new_obj = LObject(obj.0.as_ref().map(|o| o.clone_box().scale(&na::Vector3::new(x, y, z))));
                stack.replace(ctx, wrap_object(ctx, new_obj));
                Ok(CallbackReturn::Return)
            }),
        )
        .unwrap();

    methods
        .set(
            ctx,
            "clone",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let obj = ud.downcast_static::<LObject>()?;
                let new_obj = LObject(obj.0.as_ref().map(|o| o.clone_box()));
                stack.replace(ctx, wrap_object(ctx, new_obj));
                Ok(CallbackReturn::Return)
            }),
        )
        .unwrap();

    let metatable = Table::new(&ctx);
    metatable.set(ctx, MetaMethod::Index, methods).unwrap();
    ctx.set_global("__lobj_mt", metatable).unwrap();
}

/// Register custom `print` that appends to `buffer` instead of writing to stdout.
fn setup_print(ctx: Context<'_>, buffer: Arc<Mutex<String>>) {
    ctx.set_global(
        "print",
        Callback::from_fn(&ctx, move |_ctx, _, mut stack| {
            let mut parts = Vec::new();
            for i in 0..stack.len() {
                let s = match stack.get(i) {
                    Value::String(s) => {
                        std::str::from_utf8(s.as_bytes()).unwrap_or("?").to_string()
                    }
                    Value::Number(n) => n.to_string(),
                    Value::Integer(n) => n.to_string(),
                    Value::Boolean(b) => b.to_string(),
                    Value::Nil => "nil".to_string(),
                    other => other.type_name().to_string(),
                };
                parts.push(s);
            }
            buffer.lock().unwrap().push_str(&(parts.join("\t") + "\n"));
            stack.clear();
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();
}

/// Register all geometry factory functions and boolean operations as Lua globals.
fn setup_factories(ctx: Context<'_>, console: Arc<Mutex<String>>) {
    macro_rules! plane_factory {
        ($name:literal, $T:ident) => {
            ctx.set_global(
                $name,
                Callback::from_fn(&ctx, |ctx, _, mut stack| {
                    let d: Float = stack.consume(ctx)?;
                    stack.replace(ctx, wrap_object(ctx, LObject(Some(Box::new($T::new(d))))));
                    Ok(CallbackReturn::Return)
                }),
            )
            .unwrap();
        };
    }

    plane_factory!("PlaneX", PlaneX);
    plane_factory!("PlaneY", PlaneY);
    plane_factory!("PlaneZ", PlaneZ);
    plane_factory!("PlaneNegX", PlaneNegX);
    plane_factory!("PlaneNegY", PlaneNegY);
    plane_factory!("PlaneNegZ", PlaneNegZ);

    ctx.set_global(
        "Sphere",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let r: Float = stack.consume(ctx)?;
            stack.replace(ctx, wrap_object(ctx, LObject(Some(Box::new(Sphere::new(r))))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "iCylinder",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let r: Float = stack.consume(ctx)?;
            stack.replace(ctx, wrap_object(ctx, LObject(Some(Box::new(Cylinder::new(r))))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "iCone",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let slope: Float = stack.consume(ctx)?;
            stack.replace(
                ctx,
                wrap_object(ctx, LObject(Some(Box::new(Cone::new(slope, 0.0))))),
            );
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // __Box(x, y, z, smooth) — called by the Lua Box() wrapper
    ctx.set_global(
        "__Box",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (x, y, z, smooth): (Float, Float, Float, Float) = stack.consume(ctx)?;
            let obj = Intersection::from_vec(
                vec![
                    Box::new(PlaneX::new(x / 2.0)) as Box<dyn Object<Float>>,
                    Box::new(PlaneY::new(y / 2.0)),
                    Box::new(PlaneZ::new(z / 2.0)),
                    Box::new(PlaneNegX::new(x / 2.0)),
                    Box::new(PlaneNegY::new(y / 2.0)),
                    Box::new(PlaneNegZ::new(z / 2.0)),
                ],
                smooth,
            )
            .unwrap();
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // __Cylinder(length, r1, r2, smooth) — called by the Lua Cylinder() wrapper
    ctx.set_global(
        "__Cylinder",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (length, radius1, radius2, smooth): (Float, Float, Float, Float) =
                stack.consume(ctx)?;
            let conie: Box<dyn Object<Float>> = if (radius1 - radius2).abs() < EPSILON {
                Box::new(Cylinder::new(radius1))
            } else {
                let slope = (radius2 - radius1).abs() / length;
                let offset = if radius1 < radius2 {
                    -radius1 / slope - length * 0.5
                } else {
                    radius2 / slope + length * 0.5
                };
                let mut c: Box<dyn Object<Float>> = Box::new(Cone::new(slope, offset));
                let rmax = radius1.max(radius2);
                c.set_bbox(&BoundingBox::new(
                    &na::Point3::new(-rmax, -rmax, -1e10),
                    &na::Point3::new(rmax, rmax, 1e10),
                ));
                c
            };
            let obj = Intersection::from_vec(
                vec![conie, Box::new(PlaneZ::new(length / 2.0)), Box::new(PlaneNegZ::new(length / 2.0))],
                smooth,
            )
            .unwrap();
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // __PlaneHessian(nx, ny, nz, p) — called by the Lua PlaneHessian() wrapper
    ctx.set_global(
        "__PlaneHessian",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (nx, ny, nz, p): (Float, Float, Float, Float) = stack.consume(ctx)?;
            let plane = NormalPlane::from_normal_and_p(na::Vector3::new(nx, ny, nz), p);
            stack.replace(ctx, wrap_object(ctx, LObject(Some(Box::new(plane)))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // __Plane3Points(ax,ay,az, bx,by,bz, cx,cy,cz) — called by the Lua Plane3Points() wrapper
    ctx.set_global(
        "__Plane3Points",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (ax, ay, az, bx, by, bz, cx, cy, cz): (
                Float, Float, Float,
                Float, Float, Float,
                Float, Float, Float,
            ) = stack.consume(ctx)?;
            let plane = NormalPlane::from_3_points(
                &na::Point3::new(ax, ay, az),
                &na::Point3::new(bx, by, bz),
                &na::Point3::new(cx, cy, cz),
            );
            stack.replace(ctx, wrap_object(ctx, LObject(Some(Box::new(plane)))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Bend",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let ud: UserData = stack.from_front(ctx)?;
            let width: Float = stack.consume(ctx)?;
            let obj = ud.downcast_static::<LObject>()?;
            let new_obj = LObject(
                obj.0
                    .as_ref()
                    .map(|o| Box::new(Bender::new(o.clone_box(), width)) as Box<dyn Object<Float>>),
            );
            stack.replace(ctx, wrap_object(ctx, new_obj));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Twist",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let ud: UserData = stack.from_front(ctx)?;
            let height: Float = stack.consume(ctx)?;
            let obj = ud.downcast_static::<LObject>()?;
            let new_obj = LObject(
                obj.0
                    .as_ref()
                    .map(|o| Box::new(Twister::new(o.clone_box(), height)) as Box<dyn Object<Float>>),
            );
            stack.replace(ctx, wrap_object(ctx, new_obj));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Mesh",
        Callback::from_fn(&ctx, move |ctx, _, mut stack| {
            let filename: piccolo::String = stack.consume(ctx)?;
            let filename_str = std::str::from_utf8(filename.as_bytes())
                .unwrap_or("")
                .to_string();
            let obj = match Mesh::try_new(&filename_str) {
                Ok(mesh) => {
                    console
                        .lock()
                        .unwrap()
                        .push_str("Warning: Mesh support is currently horribly inefficient!\n");
                    LObject(Some(Box::new(mesh)))
                }
                Err(e) => {
                    console
                        .lock()
                        .unwrap()
                        .push_str(&format!("Could not read mesh: {e}\n"));
                    LObject(None)
                }
            };
            stack.replace(ctx, wrap_object(ctx, obj));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // Union / Intersection / Difference take (table, smooth?)
    ctx.set_global(
        "Union",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (table, smooth): (Table, Option<Float>) = stack.consume(ctx)?;
            let objects = objects_from_table(ctx, table)?;
            let obj = Union::from_vec(objects, smooth.unwrap_or(0.0))
                .ok_or_else(|| "Union requires at least one object".into_value(ctx))?;
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Intersection",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (table, smooth): (Table, Option<Float>) = stack.consume(ctx)?;
            let objects = objects_from_table(ctx, table)?;
            let obj = Intersection::from_vec(objects, smooth.unwrap_or(0.0))
                .ok_or_else(|| "Intersection requires at least one object".into_value(ctx))?;
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Difference",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (table, smooth): (Table, Option<Float>) = stack.consume(ctx)?;
            let objects = objects_from_table(ctx, table)?;
            let obj = Intersection::difference_from_vec(objects, smooth.unwrap_or(0.0))
                .ok_or_else(|| "Difference requires at least one object".into_value(ctx))?;
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();
}

/// Lua helper functions that wrap the low-level `__Box`, `__Cylinder`, etc. callbacks.
const LUA_ALIASES: &str = r#"
function Box(x, y, z, smooth)
    if type(x) ~= "number" or type(y) ~= "number" or type(z) ~= "number" then
        error("all arguments must be numbers")
    end
    local s = 0
    if type(smooth) == "number" then s = smooth end
    return __Box(x, y, z, s)
end

function Cylinder(arg)
    if type(arg.l) ~= "number" then error("l must be a valid number") end
    local r1, r2
    if type(arg.r) == "number" then
        r1, r2 = arg.r, arg.r
    elseif type(arg.r1) == "number" and type(arg.r2) == "number" then
        r1, r2 = arg.r1, arg.r2
    else
        error("specify either r or r1 and r2")
    end
    local s = 0
    if type(arg.s) == "number" then s = arg.s end
    return __Cylinder(arg.l, r1, r2, s)
end

function Plane3Points(a, b, c)
    if type(a) ~= "table" or type(b) ~= "table" or type(c) ~= "table" or
        #a ~= 3 or #b ~= 3 or #c ~= 3 then
        error("all three arguments must be tables of len 3")
    end
    for i = 1, 3 do
        if type(a[i]) ~= "number" or type(b[i]) ~= "number" or type(c[i]) ~= "number" then
            error("all table elements must be numbers")
        end
    end
    return __Plane3Points(a[1],a[2],a[3], b[1],b[2],b[3], c[1],c[2],c[3])
end

function PlaneHessian(n, p)
    if type(n) ~= "table" or #n ~= 3 or
        type(n[1]) ~= "number" or type(n[2]) ~= "number" or type(n[3]) ~= "number" then
        error("first argument (normal) must be a table of 3 numbers")
    end
    if type(p) ~= "number" then
        error("second argument must be a number (p in hessian form)")
    end
    return __PlaneHessian(n[1], n[2], n[3], p)
end
"#;

// ── public API ────────────────────────────────────────────────────────────────

/// Evaluate a Lua script in a sandboxed environment with all geometry primitives available.
///
/// Returns `(print_output, Option<built_object>)`.
pub fn eval(script: &str) -> EvalResult {
    // Lua::core() loads: base, coroutine, math, string, table — no I/O, no require, no load
    let mut lua = Lua::core();

    let print_buffer: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let result: Arc<Mutex<Option<Box<dyn Object<Float>>>>> = Arc::new(Mutex::new(None));

    {
        let print_buffer = print_buffer.clone();
        let result = result.clone();

        lua.try_enter(|ctx| {
            setup_methods_metatable(ctx);
            setup_print(ctx, print_buffer.clone());
            setup_factories(ctx, print_buffer);

            let result = result.clone();
            ctx.set_global(
                "build",
                Callback::from_fn(&ctx, move |ctx, _, mut stack| {
                    let ud: UserData = stack.from_front(ctx)?;
                    let obj = ud.downcast_static::<LObject>()?;
                    *result.lock().unwrap() = obj.as_object();
                    stack.clear();
                    Ok(CallbackReturn::Return)
                }),
            )?;

            Ok(())
        })?;
    }

    // Load Lua alias helpers
    let aliases_exec = lua.try_enter(|ctx| {
        let closure = Closure::load(ctx, None, LUA_ALIASES.as_bytes())?;
        Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
    })?;
    lua.execute::<()>(&aliases_exec)?;

    // Run the user script
    let user_exec = lua.try_enter(|ctx| {
        let closure = Closure::load(ctx, Some("script"), script.as_bytes())?;
        Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
    })?;
    lua.execute::<()>(&user_exec)?;

    let output = print_buffer.lock().unwrap().clone();
    let obj = result.lock().unwrap().take();
    Ok((output, obj))
}
