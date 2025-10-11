use std::{
    collections::HashMap,
    fs::{self, File},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use color_eyre::eyre::{self, eyre};
use minijinja::Value as JValue;
use mlua::{FromLuaMulti, IntoLua, Lua, LuaSerdeExt, Value};

use crate::paths;

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

fn try_new_shebang() -> eyre::Result<Shebang> {
    let lua = Lua::new();

    lua.set_app_data(State {
        render_queue: Vec::new(),
        global_context: HashMap::new(),
    });

    lua.register(
        "render",
        move |lua, (template, target, context): (String, String, Value)| {
            trace!("lua render: {:?} {:?} => {:?}", template, target, context);

            let mut state = lua.app_data_mut::<State>().unwrap();
            state.render_queue.push(Render {
                context: JValue::from_serialize(context),
                target: paths::dist()?.join(target),
                template,
            });

            Ok(Value::Nil)
        },
    )?;

    lua.register("json", move |lua, path: String| {
        let path = paths::www()?.join(path);
        let file = File::open(path)?;
        let serde: JValue = serde_json::from_reader(file)?; // INSANE hack
        Ok(lua.to_value(&serde)?)
    })?;

    lua.register("read", move |_, path: String| {
        let path = paths::www()?.join(path);
        Ok(fs::read_to_string(&path)?)
    })?;

    lua.register("lastmod", move |_, path: String| {
        let path = paths::www()?.join(path);
        let modif = fs::metadata(&path).and_then(|x| x.modified())?;
        Ok(fmt_iso(modif))
    })?;

    lua.register("inject", move |lua, (name, value): (String, Value)| {
        lua.app_data_mut::<State>()
            .unwrap()
            .global_context
            .insert(name, JValue::from_serialize(value));
        Ok(Value::Nil)
    })?;

    Ok(Shebang { lua })
}

pub fn write_lualib() {
    match write_lualib_inner() {
        Ok(()) => info!("Wrote _sanity.lua"),
        Err(err) => error!("Failed to write _sanity.lua: {}", err),
    }
}

fn write_lualib_inner() -> eyre::Result<()> {
    fs::write(paths::root()?.join("_sanity.lua"), include_str!("lib.lua"))?;
    Ok(())
}

trait LuaRegisterExt {
    fn register<Arg, T>(
        &self,
        name: &str,
        func: impl Fn(&Lua, Arg) -> eyre::Result<T> + Send + 'static,
    ) -> eyre::Result<()>
    where
        Arg: FromLuaMulti + 'static,
        T: IntoLua + 'static;
}

impl LuaRegisterExt for Lua {
    fn register<Arg, T>(
        &self,
        name: &str,
        func: impl Fn(&Lua, Arg) -> eyre::Result<T> + Send + 'static,
    ) -> eyre::Result<()>
    where
        Arg: FromLuaMulti + 'static,
        T: IntoLua + 'static,
    {
        let cls = move |lua: &Lua, arg: Arg| match func(lua, arg)
            .and_then(|x| x.into_lua(lua).map_err(|err| eyre!("{:?}", err)))
        {
            Ok(v) => Ok(v),
            Err(err) => {
                error!("Lua error: {:?}", err);
                Ok(Value::Nil)
            }
        };
        self.globals().set(name, self.create_function(cls)?)?;
        Ok(())
    }
}

fn fmt_iso(datetime: impl Into<DateTime<Utc>>) -> String {
    datetime.into().format("%+").to_string()
}
