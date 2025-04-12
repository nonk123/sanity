use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use mlua::{Lua, LuaSerdeExt, Value};

use crate::{Result, paths};

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
    pub fn process(&self, file: &Path) -> Result<()> {
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

pub fn new() -> Result<Shebang> {
    let lua = Lua::new();
    let globals = lua.globals();

    let state0 = Arc::new(Mutex::new(State::new()));

    let state = state0.clone();
    let render = lua.create_function(
        move |_, (template, target, context): (String, String, Value)| {
            trace!("lua render: {:?} {:?} => {:?}", template, target, context);

            state.lock().unwrap().render_queue.push(Render {
                context: minijinja::Value::from_serialize(context),
                target: paths::dist().unwrap().join(target),
                template,
            });

            Ok(())
        },
    )?;
    globals.set("render", render)?;

    let json = lua.create_function(move |lua, path: String| {
        fn inner(lua: &Lua, path: &Path) -> Result<Value> {
            let file = File::open(path)?;
            let serde: minijinja::Value = serde_json::from_reader(file)?; // INSANE hack
            Ok(lua.to_value(&serde)?)
        }

        let path = paths::www().unwrap().join(path);
        match inner(lua, &path) {
            Ok(ok) => Ok(ok),
            Err(err) => {
                error!("JSON load failed {:?}: {:?}", path, err);
                Ok(Value::Nil)
            }
        }
    })?;
    globals.set("json", json)?;

    Ok(Shebang { lua, state: state0 })
}
