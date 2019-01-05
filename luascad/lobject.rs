use super::{Float, EPSILON};
use hlua;
use implicit3d::{
    Bender, BoundingBox, Cone, Cylinder, Intersection, Mesh, Object, PlaneNegX, PlaneNegY,
    PlaneNegZ, PlaneX, PlaneY, PlaneZ, Sphere, Twister,
};
use nalgebra as na;
use std::sync::mpsc;

#[derive(Clone, Debug)]
pub struct LObject {
    pub o: Option<Box<Object<Float>>>,
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
    pub fn as_object(&self) -> Option<Box<Object<Float>>> {
        self.o.clone()
    }
    fn add_aliases(lua: &mut hlua::Lua, env_name: &str) {
        lua.execute::<()>(&format!(
            r#"
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
                return __Cylinder(r1, r2, arg.l, s)
            end
            {env}.Cylinder = Cylinder;
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
                "Box",
                hlua::function4(
                    |x: Float, y: Float, z: Float, smooth_lua: hlua::AnyLuaValue| {
                        let mut smooth = 0.;
                        if let hlua::AnyLuaValue::LuaNumber(v) = smooth_lua {
                            smooth = v;
                        }
                        LObject {
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
                        }
                    },
                ),
            );
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
            "__Cylinder",
            hlua::function4(
                |length: Float, radius1: Float, radius2: Float, smooth: Float| {
                    let mut conie;
                    if (radius1 - radius2).abs() < EPSILON {
                        conie = Box::new(Cylinder::new(radius1)) as Box<Object<Float>>;
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
