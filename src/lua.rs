use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use mlua::{Lua, Value};

pub struct Render {
    pub template: String,
    pub target: PathBuf,
    pub context: minijinja::Value,
}

pub struct Shebang {
    lua: Lua,
    pub state: Arc<Mutex<State>>,
}

impl Shebang {
    pub fn process(&self, file: &Path) -> crate::Result<()> {
        self.lua.load(file).exec()?;
        Ok(())
    }

    pub fn postbuild_cleanup(&self) {
        *self.state.lock().unwrap() = State::new();
    }
}

pub struct State {
    pub render_queue: Vec<Render>,
}

impl State {
    fn new() -> Self {
        Self {
            render_queue: Vec::new(),
        }
    }
}

pub fn new() -> crate::Result<Shebang> {
    let lua = Lua::new();
    let globals = lua.globals();

    let state0 = Arc::new(Mutex::new(State::new()));

    let state = state0.clone();
    let render = lua.create_function(
        move |_, (template, target, context): (String, String, Value)| {
            trace!("lua render: {:?} {:?} => {:?}", template, target, context);

            state.lock().unwrap().render_queue.push(Render {
                context: minijinja::Value::from_serialize(context),
                target: crate::paths::dist().unwrap().join(target),
                template,
            });

            Ok(())
        },
    )?;
    globals.set("render", render)?;

    Ok(Shebang { lua, state: state0 })
}
