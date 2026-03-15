use super::Float;
use hlua;
use hlua::{Lua, LuaError};
use crate::lobject::LObject;
use crate::lobject_vector::LObjectVector;
use crate::printbuffer;
use crate::sandbox;

pub const USER_FUNCTION_NAME: &str = "__luscad_user_function__";
pub const SANDBOX_ENV_NAME: &str = "__luascad_sandbox_env__";

pub type EvalResult = Result<(String, Option<Box<dyn implicit3d::Object<Float>>>), LuaError>;

pub fn eval(script: &str) -> EvalResult {
    let mut result = None;
    let print_output;
    {
        let mut lua = Lua::new();
        lua.openlibs();
        sandbox::set_sandbox_env(&mut lua, SANDBOX_ENV_NAME);
        let printbuffer =
            printbuffer::PrintBuffer::new_and_expose_to_lua(&mut lua, SANDBOX_ENV_NAME);
        {
            let mut sandbox_env = lua.get::<hlua::LuaTable<_>, _>(SANDBOX_ENV_NAME).unwrap();
            sandbox_env.set(
                "build",
                hlua::function1(|o: &LObject| result = o.as_object()),
            );
        }
        LObject::export_factories(&mut lua, SANDBOX_ENV_NAME, printbuffer.get_tx());
        LObjectVector::export_factories(&mut lua, SANDBOX_ENV_NAME);

        lua.checked_set(USER_FUNCTION_NAME, hlua::LuaCode(script))?;
        lua.execute::<()>(&format!(
            "debug.setupvalue({}, 1, {}); return {}();",
            USER_FUNCTION_NAME, SANDBOX_ENV_NAME, USER_FUNCTION_NAME
        ))?;
        print_output = printbuffer.get_buffer();
    }
    Ok((print_output, result))
}
