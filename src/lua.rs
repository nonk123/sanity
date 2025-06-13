use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
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
}

impl Shebang {
    pub fn try_new() -> Result<Self> {
        try_new_shebang()
    }

    pub fn process(&self, file: &Path) -> Result<()> {
        self.lua.load(file).exec()?;
        Ok(())
    }

    pub fn state(&self) -> State {
        self.lua.remove_app_data().unwrap()
    }
}

pub struct State {
    pub render_queue: Vec<Render>,
    pub global_context: HashMap<String, JValue>,
}

fn try_new_shebang() -> Result<Shebang> {
    let lua = Lua::new();
    let globals = lua.globals();

    lua.set_app_data(State {
        render_queue: Vec::new(),
        global_context: HashMap::new(),
    });

    let render = lua.create_function(
        move |lua, (template, target, context): (String, String, Value)| {
            trace!("lua render: {:?} {:?} => {:?}", template, target, context);

            let mut state = lua.app_data_mut::<State>().unwrap();
            state.render_queue.push(Render {
                context: JValue::from_serialize(context),
                target: paths::dist().unwrap().join(target),
                template,
            });

            Ok(())
        },
    )?;
    globals.set("render", render)?;

    let json = lua.create_function(move |lua, path: String| {
        fn inner(lua: &Lua, path: &str) -> Result<Value> {
            let path = paths::www().unwrap().join(path);
            let file = File::open(path)?;
            let serde: JValue = serde_json::from_reader(file)?; // INSANE hack
            Ok(lua.to_value(&serde)?)
        }

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

    let inject = lua.create_function(move |lua, (name, value): (String, Value)| {
        lua.app_data_mut::<State>()
            .unwrap()
            .global_context
            .insert(name, JValue::from_serialize(value));

        Ok(Value::Nil)
    })?;
    globals.set("inject", inject)?;

    Ok(Shebang { lua })
}
