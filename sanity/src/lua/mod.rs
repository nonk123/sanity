use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use color_eyre::eyre::{self, eyre};
use minijinja::Value as JValue;
use mlua::{FromLua, FromLuaMulti, IntoLua, Lua, Value};

use crate::paths;

mod fns;

pub struct Render {
    pub template: String,
    pub target: PathBuf,
    pub context: JValue,
}

pub struct Shebang {
    lua: Lua,
}

impl Shebang {
    pub fn try_new() -> eyre::Result<Self> {
        try_new_shebang()
    }

    pub fn process(&self, file: &Path) -> eyre::Result<()> {
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

pub trait LuaFn {
    fn call(&self, lua: &mlua::Lua, args: mlua::MultiValue) -> eyre::Result<Value>;
    fn name(&self) -> String;
    fn docs(&self) -> Vec<String>;
    fn params(&self) -> Vec<String>;
}

fn try_new_shebang() -> eyre::Result<Shebang> {
    let lua = Lua::new();

    lua.set_app_data(State {
        render_queue: Vec::new(),
        global_context: HashMap::new(),
    });

    for fun in fns::all() {
        let name = fun.name();

        let cls = move |lua: &Lua, args| match fun
            .call(lua, args)
            .and_then(|x| x.into_lua(lua).map_err(|err| eyre!("{:?}", err)))
        {
            Ok(v) => Ok(v),
            Err(err) => {
                error!("Lua error: {:?}", err);
                Ok(Value::Nil)
            }
        };

        lua.globals().set(name, lua.create_function(cls)?)?;
    }

    Ok(Shebang { lua })
}

pub fn write_lualib() {
    match write_lualib_inner() {
        Ok(()) => info!("Wrote _sanity.lua"),
        Err(err) => error!("Failed to write _sanity.lua: {}", err),
    }
}

fn write_lualib_inner() -> eyre::Result<()> {
    let file = File::create(paths::root()?.join("_sanity.lua"))?;

    for fun in fns::all() {}

    Ok(())
}
