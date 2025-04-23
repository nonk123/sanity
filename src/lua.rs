use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use minijinja::Value as JValue;
use mlua::{Lua, LuaSerdeExt, Value};

use crate::{Result, paths};

pub struct Render {
    pub template: String,
    pub target: PathBuf,
    pub context: JValue,
}

pub struct Shebang {
    lua: Lua,
    pub state: Arc<Mutex<State>>,
}

impl Shebang {
    pub fn try_new() -> Result<Self> {
        try_new_shebang()
    }

    pub fn process(&self, file: &Path) -> Result<()> {
        self.lua.load(file).exec()?;
        Ok(())
    }
}

pub struct State {
    pub render_queue: Vec<Render>,
    pub global_context: HashMap<String, JValue>,
}

fn try_new_shebang() -> Result<Shebang> {
    let lua = Lua::new();
    let globals = lua.globals();

    let _state = Arc::new(Mutex::new(State {
        render_queue: Vec::new(),
        global_context: HashMap::new(),
    }));

    let state = Arc::downgrade(&_state);
    let render = lua.create_function(
        move |_, (template, target, context): (String, String, Value)| {
            let Some(state) = state.upgrade() else {
                unreachable!();
            };

            trace!("lua render: {:?} {:?} => {:?}", template, target, context);

            state.lock().unwrap().render_queue.push(Render {
                context: JValue::from_serialize(context),
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
            let serde: JValue = serde_json::from_reader(file)?; // INSANE hack
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

    let read = lua.create_function(move |lua, path: String| {
        let path = paths::www().unwrap().join(path);
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                let s = lua.create_string(s).unwrap();
                Ok(Value::String(s))
            }
            Err(err) => {
                error!("read failed {:?}: {:?}", path, err);
                Ok(Value::Nil)
            }
        }
    })?;
    globals.set("read", read)?;

    let state = Arc::downgrade(&_state);
    let inject = lua.create_function(move |_, (name, value): (String, Value)| {
        let Some(state) = state.upgrade() else {
            unreachable!();
        };

        state
            .lock()
            .unwrap()
            .global_context
            .insert(name, JValue::from_serialize(value));

        Ok(Value::Nil)
    })?;
    globals.set("inject", inject)?;

    Ok(Shebang { lua, state: _state })
}
