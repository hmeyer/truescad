use std::sync::{Arc, Mutex};

use piccolo::{
    Callback, CallbackReturn, Closure, Context, Executor, IntoValue, Lua, MetaMethod, Table,
    UserData, Value,
};

use crate::primitive::Primitive;
use crate::primitives::{
    Bender, Difference, InfCone, InfCylinder, Intersection, NormalPlane, PlaneNegX, PlaneNegY,
    PlaneNegZ, PlaneX, PlaneY, PlaneZ, Rotate, Scale, Sphere, Translate, Twister, Union,
};

const EPSILON: f64 = f64::EPSILON;

pub type EvalResult = Result<(String, Option<Box<dyn Primitive>>), piccolo::StaticError>;

/// Lua-visible wrapper around a Primitive.
pub struct LObject(pub Option<Box<dyn Primitive>>);

impl LObject {
    fn as_primitive(&self) -> Option<Box<dyn Primitive>> {
        self.0.as_ref().map(|o| o.clone_box())
    }
}

// ── internal helpers ──────────────────────────────────────────────────────────

fn wrap_object(ctx: Context<'_>, obj: LObject) -> Value<'_> {
    let ud = UserData::new_static(&ctx, obj);
    if let Value::Table(mt) = ctx.get_global("__lobj_mt") {
        ud.set_metatable(&ctx, Some(mt));
    }
    ud.into()
}

fn objects_from_table<'gc>(
    ctx: Context<'gc>,
    table: Table<'gc>,
) -> Result<Vec<Box<dyn Primitive>>, piccolo::Error<'gc>> {
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
                    None => return Err("nil object in list".into_value(ctx).into()),
                }
            }
            _ => return Err("expected LObject in list".into_value(ctx).into()),
        }
    }
    Ok(objects)
}

// ── setup functions ───────────────────────────────────────────────────────────

fn setup_methods_metatable(ctx: Context<'_>) {
    let methods = Table::new(&ctx);

    methods
        .set(
            ctx,
            "translate",
            Callback::from_fn(&ctx, |ctx, _, mut stack| {
                let ud: UserData = stack.from_front(ctx)?;
                let (x, y, z): (f64, f64, f64) = stack.consume(ctx)?;
                let obj = ud.downcast_static::<LObject>()?;
                let new_obj = LObject(obj.0.as_ref().map(|o| {
                    Box::new(Translate::new(o.clone_box(), [x as f32, y as f32, z as f32]))
                        as Box<dyn Primitive>
                }));
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
                let (x, y, z): (f64, f64, f64) = stack.consume(ctx)?;
                let obj = ud.downcast_static::<LObject>()?;
                let new_obj = LObject(obj.0.as_ref().map(|o| {
                    Box::new(Rotate::new(o.clone_box(), [x as f32, y as f32, z as f32]))
                        as Box<dyn Primitive>
                }));
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
                let (x, y, z): (f64, f64, f64) = stack.consume(ctx)?;
                let obj = ud.downcast_static::<LObject>()?;
                let new_obj = LObject(obj.0.as_ref().map(|o| {
                    Box::new(Scale::new(o.clone_box(), [x as f32, y as f32, z as f32]))
                        as Box<dyn Primitive>
                }));
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

fn setup_print(ctx: Context<'_>, buffer: Arc<Mutex<String>>) {
    ctx.set_global(
        "print",
        Callback::from_fn(&ctx, move |_ctx, _, mut stack| {
            let mut parts = Vec::new();
            for i in 0..stack.len() {
                let s = match stack.get(i) {
                    Value::String(s) => std::str::from_utf8(s.as_bytes()).unwrap_or("?").to_string(),
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

fn setup_factories(ctx: Context<'_>, console: Arc<Mutex<String>>) {
    macro_rules! plane_factory {
        ($name:literal, $T:ident) => {
            ctx.set_global(
                $name,
                Callback::from_fn(&ctx, |ctx, _, mut stack| {
                    let d: f64 = stack.consume(ctx)?;
                    stack.replace(
                        ctx,
                        wrap_object(ctx, LObject(Some(Box::new($T::new(d as f32))))),
                    );
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
            let r: f64 = stack.consume(ctx)?;
            stack.replace(ctx, wrap_object(ctx, LObject(Some(Box::new(Sphere::new(r as f32))))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "iCylinder",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let r: f64 = stack.consume(ctx)?;
            stack.replace(
                ctx,
                wrap_object(ctx, LObject(Some(Box::new(InfCylinder::new(r as f32))))),
            );
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "iCone",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let slope: f64 = stack.consume(ctx)?;
            stack.replace(
                ctx,
                wrap_object(ctx, LObject(Some(Box::new(InfCone::new(slope as f32, 0.0))))),
            );
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // __Box(x, y, z, smooth) — 6-plane intersection
    ctx.set_global(
        "__Box",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (x, y, z, smooth): (f64, f64, f64, f64) = stack.consume(ctx)?;
            let (x, y, z, smooth) = (x as f32, y as f32, z as f32, smooth as f32);
            let children: Vec<Box<dyn Primitive>> = vec![
                Box::new(PlaneX::new(x / 2.0)),
                Box::new(PlaneY::new(y / 2.0)),
                Box::new(PlaneZ::new(z / 2.0)),
                Box::new(PlaneNegX::new(x / 2.0)),
                Box::new(PlaneNegY::new(y / 2.0)),
                Box::new(PlaneNegZ::new(z / 2.0)),
            ];
            let obj: Box<dyn Primitive> = Box::new(Intersection::new(children, smooth));
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // __Cylinder(length, r1, r2, smooth)
    ctx.set_global(
        "__Cylinder",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (length, radius1, radius2, smooth): (f64, f64, f64, f64) = stack.consume(ctx)?;
            let (length, radius1, radius2, smooth) =
                (length as f32, radius1 as f32, radius2 as f32, smooth as f32);

            let shaft: Box<dyn Primitive> = if (radius1 - radius2).abs() < EPSILON as f32 {
                Box::new(InfCylinder::new(radius1))
            } else {
                let slope = (radius2 - radius1).abs() / length;
                let offset = if radius1 < radius2 {
                    -radius1 / slope - length * 0.5
                } else {
                    radius2 / slope + length * 0.5
                };
                Box::new(InfCone::new(slope, offset))
            };

            let children: Vec<Box<dyn Primitive>> = vec![
                shaft,
                Box::new(PlaneZ::new(length / 2.0)),
                Box::new(PlaneNegZ::new(length / 2.0)),
            ];
            let obj: Box<dyn Primitive> = Box::new(Intersection::new(children, smooth));
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // __PlaneHessian(nx, ny, nz, p)
    ctx.set_global(
        "__PlaneHessian",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (nx, ny, nz, p): (f64, f64, f64, f64) = stack.consume(ctx)?;
            let plane = NormalPlane::from_normal_and_p(
                [nx as f32, ny as f32, nz as f32],
                p as f32,
            );
            stack.replace(ctx, wrap_object(ctx, LObject(Some(Box::new(plane)))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    // __Plane3Points(ax,ay,az, bx,by,bz, cx,cy,cz)
    ctx.set_global(
        "__Plane3Points",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (ax, ay, az, bx, by, bz, cx, cy, cz): (
                f64, f64, f64,
                f64, f64, f64,
                f64, f64, f64,
            ) = stack.consume(ctx)?;
            let plane = NormalPlane::from_3_points(
                [ax as f32, ay as f32, az as f32],
                [bx as f32, by as f32, bz as f32],
                [cx as f32, cy as f32, cz as f32],
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
            let width: f64 = stack.consume(ctx)?;
            let obj = ud.downcast_static::<LObject>()?;
            let new_obj = LObject(obj.0.as_ref().map(|o| {
                Box::new(Bender::new(o.clone_box(), width as f32)) as Box<dyn Primitive>
            }));
            stack.replace(ctx, wrap_object(ctx, new_obj));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Twist",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let ud: UserData = stack.from_front(ctx)?;
            let height: f64 = stack.consume(ctx)?;
            let obj = ud.downcast_static::<LObject>()?;
            let new_obj = LObject(obj.0.as_ref().map(|o| {
                Box::new(Twister::new(o.clone_box(), height as f32)) as Box<dyn Primitive>
            }));
            stack.replace(ctx, wrap_object(ctx, new_obj));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Mesh",
        Callback::from_fn(&ctx, move |ctx, _, mut stack| {
            let _filename: piccolo::String = stack.consume(ctx)?;
            console
                .lock()
                .unwrap()
                .push_str("Mesh not supported in GPU mode.\n");
            stack.replace(ctx, wrap_object(ctx, LObject(None)));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Union",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (table, smooth): (Table, Option<f64>) = stack.consume(ctx)?;
            let objects = objects_from_table(ctx, table)?;
            if objects.is_empty() {
                return Err("Union requires at least one object".into_value(ctx).into());
            }
            let obj: Box<dyn Primitive> =
                Box::new(Union::new(objects, smooth.unwrap_or(0.0) as f32));
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Intersection",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (table, smooth): (Table, Option<f64>) = stack.consume(ctx)?;
            let objects = objects_from_table(ctx, table)?;
            if objects.is_empty() {
                return Err("Intersection requires at least one object".into_value(ctx).into());
            }
            let obj: Box<dyn Primitive> =
                Box::new(Intersection::new(objects, smooth.unwrap_or(0.0) as f32));
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();

    ctx.set_global(
        "Difference",
        Callback::from_fn(&ctx, |ctx, _, mut stack| {
            let (table, smooth): (Table, Option<f64>) = stack.consume(ctx)?;
            let objects = objects_from_table(ctx, table)?;
            if objects.is_empty() {
                return Err("Difference requires at least one object".into_value(ctx).into());
            }
            let obj: Box<dyn Primitive> =
                Box::new(Difference::new(objects, smooth.unwrap_or(0.0) as f32));
            stack.replace(ctx, wrap_object(ctx, LObject(Some(obj))));
            Ok(CallbackReturn::Return)
        }),
    )
    .unwrap();
}

const LUA_ALIASES: &str = r#"
pi  = 3.14159265358979323846
tau = 6.28318530717958647692

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

pub fn eval(script: &str) -> EvalResult {
    let mut lua = Lua::core();

    let print_buffer: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let result: Arc<Mutex<Option<Box<dyn Primitive>>>> = Arc::new(Mutex::new(None));

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
                    *result.lock().unwrap() = obj.as_primitive();
                    stack.clear();
                    Ok(CallbackReturn::Return)
                }),
            )?;

            Ok(())
        })?;
    }

    let aliases_exec = lua.try_enter(|ctx| {
        let closure = Closure::load(ctx, None, LUA_ALIASES.as_bytes())?;
        Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
    })?;
    lua.execute::<()>(&aliases_exec)?;

    let user_exec = lua.try_enter(|ctx| {
        let closure = Closure::load(ctx, Some("script"), script.as_bytes())?;
        Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
    })?;
    lua.execute::<()>(&user_exec)?;

    let output = print_buffer.lock().unwrap().clone();
    let obj = result.lock().unwrap().take();
    Ok((output, obj))
}
