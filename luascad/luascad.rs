use super::Float;
use crate::lobject;
use piccolo::{Closure, Executor, Lua, StaticError};
use std::cell::RefCell;
use std::rc::Rc;

pub type EvalResult = Result<(String, Option<Box<dyn implicit3d::Object<Float>>>), StaticError>;

pub fn eval(script: &str) -> EvalResult {
    let mut lua = Lua::core();

    // Shared state for capturing build() result and print output
    let build_result: Rc<RefCell<Option<Box<dyn implicit3d::Object<Float>>>>> =
        Rc::new(RefCell::new(None));
    let print_buffer: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    // Register all object constructors and the build/print functions
    lua.try_enter(|ctx| {
        lobject::register_all(ctx, build_result.clone(), print_buffer.clone())?;
        Ok(())
    })?;

    // Compile and run the user script
    let executor = lua.try_enter(|ctx| {
        let closure = Closure::load(ctx, None, script.as_bytes())?;
        Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
    })?;

    lua.execute::<()>(&executor)?;

    let result = build_result.borrow_mut().take();
    let output = print_buffer.borrow().clone();
    Ok((output, result))
}
