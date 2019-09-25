use super::{Float, EPSILON};
use hlua;
use implicit3d::{
    Bender, BoundingBox, Cone, Cylinder, Intersection, Mesh, NormalPlane, Object, PlaneNegX,
    PlaneNegY, PlaneNegZ, PlaneX, PlaneY, PlaneZ, Sphere, Twister,
};
use nalgebra as na;
use std::sync::mpsc;

#[derive(Clone, Debug)]
pub struct LObject {
    pub o: Option<Box<dyn Object<Float>>>,
}

pub const INFINITY: Float = 1e10;
pub const NEG_INFINITY: Float = -1e10;

// this macro implements the required trait so that we can *push* the object to lua
// (ie. move it inside lua)
implement_lua_push!(LObject, |mut metatable| {
    {
        // we create a `__index` entry in the metatable
        // when the lua code calls `object:translate()`, it will look for `translate` in there
        let mut index = metatable.empty_array("__index");

        index.set(
            "translate",
            ::hlua::function4(|o: &mut LObject, x: Float, y: Float, z: Float| o.translate(x, y, z)),
        );
        index.set(
            "rotate",
            ::hlua::function4(|o: &mut LObject, x: Float, y: Float, z: Float| o.rotate(x, y, z)),
        );
        index.set(
            "scale",
            ::hlua::function4(|o: &mut LObject, x: Float, y: Float, z: Float| o.scale(x, y, z)),
        );
        index.set("clone", ::hlua::function1(|o: &mut LObject| o.clone()));
    }
    // Add __tostring metamethod for printing LObjects.
    metatable.set(
        "__tostring",
        ::hlua::function1(|o: &mut LObject| format!("{:#?}", o)),
    );
});

// this macro implements the require traits so that we can *read* the object back
implement_lua_read!(LObject);

impl LObject {
    pub fn as_object(&self) -> Option<Box<dyn Object<Float>>> {
        self.o.clone()
    }
    fn add_aliases(lua: &mut hlua::Lua, env_name: &str) {
        lua.execute::<()>(&format!(
            r#"
            function Box (x, y, z, smooth)
                if type(x) ~= "number" or type(x) ~= "number" or type(y) ~= "number" then
                    error("all arguments must be numbers")
                end
                s = 0
                if type(smooth) == "number" then
                    s = smooth
                end
                return __Box(x, y, z, s)
            end
            function Cylinder (arg)
                if type(arg.l) ~= "number" then
                    error("l must be a valid number")
                end
                if type(arg.r) == "number" then
                    r1 = arg.r
                    r2 = arg.r
                elseif type(arg.r1) == "number" and type(arg.r2) == "number" then
                    r1 = arg.r1
                    r2 = arg.r2
                else
                    error("specify either r or r1 and r2")
                end
                s = 0
                if type(arg.s) == "number" then
                    s = arg.s
                end
                return __Cylinder(arg.l, r1, r2, s)
            end
            function Plane3Points (a,b,c)
                if type(a) ~= "table" or type(b) ~= "table" or type(c) ~= "table" or
                    #a ~= 3 or #b ~= 3 or #c ~= 3 then
                    error("all three arguments must be tables of len 3")
                end
                for i=1,3 do
                  if type(a[i]) ~= "number" or type(b[i]) ~= "number" or type(c[i]) ~= "number" then
                    error("all table elements must be numbers")
                  end
                end
                return __Plane3Points(a[1], a[2], a[3],
                                      b[1], b[2], b[3],
                                      c[1], c[2], c[3])
            end
            function PlaneHessian (n,p)
                if type(n) ~= "table" or #n ~= 3 or
                    type(n[1]) ~= "number" or type(n[2]) ~= "number" or type(n[3]) ~= "number" then
                    error("first argument (normal) must be a table of 3 numbers")
                end
                if type(p) ~= "number" then
                    error("second argument must be a number (p in hessian form)")
                end
                return __PlaneHessian(n[1], n[2], n[3], p)
            end
            {env}.Box = Box;
            {env}.Cylinder = Cylinder;
            {env}.Plane3Points = Plane3Points;
            {env}.PlaneHessian = PlaneHessian;
            "#,
            env = env_name
        ))
        .unwrap();
    }
    pub fn export_factories(lua: &mut hlua::Lua, env_name: &str, console: mpsc::Sender<String>) {
        {
            let mut env = lua.get::<hlua::LuaTable<_>, _>(env_name).unwrap();

            macro_rules! one_param_object {
                ( $x:ident ) => {
                    env.set(
                        stringify!($x),
                        hlua::function1(move |d_lua: hlua::AnyLuaValue| {
                            let mut d = 0.;
                            if let hlua::AnyLuaValue::LuaNumber(v) = d_lua {
                                d = v;
                            }
                            LObject {
                                o: Some(Box::new($x::new(d))),
                            }
                        }),
                    );
                };
            }

            one_param_object!(PlaneX);
            one_param_object!(PlaneY);
            one_param_object!(PlaneZ);
            one_param_object!(PlaneNegX);
            one_param_object!(PlaneNegY);
            one_param_object!(PlaneNegZ);
            env.set(
                "Sphere",
                hlua::function1(|radius: Float| LObject {
                    o: Some(Box::new(Sphere::new(radius))),
                }),
            );
            env.set(
                "iCylinder",
                hlua::function1(|radius: Float| LObject {
                    o: Some(Box::new(Cylinder::new(radius))),
                }),
            );
            env.set(
                "iCone",
                hlua::function1(|slope: Float| LObject {
                    o: Some(Box::new(Cone::new(slope, 0.))),
                }),
            );
            env.set(
                "Bend",
                hlua::function2(|o: &LObject, width: Float| LObject {
                    o: if let Some(obj) = o.as_object() {
                        Some(Box::new(Bender::new(obj, width)))
                    } else {
                        None
                    },
                }),
            );
            env.set(
                "Twist",
                hlua::function2(|o: &LObject, height: Float| LObject {
                    o: if let Some(obj) = o.as_object() {
                        Some(Box::new(Twister::new(obj, height)))
                    } else {
                        None
                    },
                }),
            );
            env.set(
                "Mesh",
                hlua::function1(move |filename: String| LObject {
                    o: match Mesh::try_new(&filename) {
                        Ok(mesh) => {
                            console
                                .send(
                                    "Warning: Mesh support is currently horribly inefficient!"
                                        .to_string(),
                                )
                                .unwrap();
                            Some(Box::new(mesh))
                        }
                        Err(e) => {
                            console
                                .send(format!("Could not read mesh: {:}", e))
                                .unwrap();
                            None
                        }
                    },
                }),
            );
        }
        lua.set(
            "__Box",
            hlua::function4(|x: Float, y: Float, z: Float, smooth: Float| LObject {
                o: Some(
                    Intersection::from_vec(
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
                    .unwrap(),
                ),
            }),
        );
        lua.set(
            "__PlaneHessian",
            hlua::function4(|nx: Float, ny: Float, nz: Float, p: Float| LObject {
                o: Some(Box::new(NormalPlane::from_normal_and_p(
                    na::Vector3::new(nx, ny, nz),
                    p,
                ))),
            }),
        );
        lua.set(
            "__Plane3Points",
            hlua::function9(
                |ax: Float,
                 ay: Float,
                 az: Float,
                 bx: Float,
                 by: Float,
                 bz: Float,
                 cx: Float,
                 cy: Float,
                 cz: Float| {
                    LObject {
                        o: Some(Box::new(NormalPlane::from_3_points(
                            &na::Point3::new(ax, ay, az),
                            &na::Point3::new(bx, by, bz),
                            &na::Point3::new(cx, cy, cz),
                        ))),
                    }
                },
            ),
        );
        lua.set(
            "__Cylinder",
            hlua::function4(
                |length: Float, radius1: Float, radius2: Float, smooth: Float| {
                    let mut conie;
                    if (radius1 - radius2).abs() < EPSILON {
                        conie = Box::new(Cylinder::new(radius1)) as Box<dyn Object<Float>>;
                    } else {
                        let slope = (radius2 - radius1).abs() / length;
                        let offset = if radius1 < radius2 {
                            -radius1 / slope - length * 0.5
                        } else {
                            radius2 / slope + length * 0.5
                        };
                        conie = Box::new(Cone::new(slope, offset));
                        let rmax = radius1.max(radius2);
                        let conie_box = BoundingBox::new(
                            &na::Point3::new(-rmax, -rmax, NEG_INFINITY),
                            &na::Point3::new(rmax, rmax, INFINITY),
                        );
                        conie.set_bbox(&conie_box);
                    }
                    LObject {
                        o: Some(
                            Intersection::from_vec(
                                vec![
                                    conie,
                                    Box::new(PlaneZ::new(length / 2.0)),
                                    Box::new(PlaneNegZ::new(length / 2.0)),
                                ],
                                smooth,
                            )
                            .unwrap(),
                        ),
                    }
                },
            ),
        );
        LObject::add_aliases(lua, env_name);
    }
    fn translate(&mut self, x: Float, y: Float, z: Float) -> LObject {
        LObject {
            o: if let Some(ref obj) = self.o {
                Some(obj.clone().translate(&na::Vector3::new(x, y, z)))
            } else {
                None
            },
        }
    }
    fn rotate(&mut self, x: Float, y: Float, z: Float) -> LObject {
        LObject {
            o: if let Some(ref obj) = self.o {
                Some(obj.clone().rotate(&na::Vector3::new(x, y, z)))
            } else {
                None
            },
        }
    }
    fn scale(&mut self, x: Float, y: Float, z: Float) -> LObject {
        LObject {
            o: if let Some(ref obj) = self.o {
                Some(obj.clone().scale(&na::Vector3::new(x, y, z)))
            } else {
                None
            },
        }
    }
}
