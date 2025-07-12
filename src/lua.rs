use std::{
    collections::HashMap,
    fs::{self, File},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use color_eyre::eyre::eyre;
use minijinja::Value as JValue;
use mlua::{IntoLua, Lua, LuaSerdeExt, Value};

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

    let render = lua.create_function(luaize(
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
    ))?;
    globals.set("render", render)?;

    let json = lua.create_function(luaize(move |lua, path: String| {
        let path = paths::www()?.join(path);
        let file = File::open(path)?;
        let serde: JValue = serde_json::from_reader(file)?; // INSANE hack
        Ok(lua.to_value(&serde)?)
    }))?;
    globals.set("json", json)?;

    let read = lua.create_function(luaize(move |_, path: String| {
        let path = paths::www()?.join(path);
        Ok(fs::read_to_string(&path)?)
    }))?;
    globals.set("read", read)?;

    let lastmod = lua.create_function(luaize(move |_, path: String| {
        let path = paths::www()?.join(path);
        let modif = fs::metadata(&path).and_then(|x| x.modified())?;
        Ok(fmt_iso(modif))
    }))?;
    globals.set("lastmod", lastmod)?;

    let inject = lua.create_function(luaize(move |lua, (name, value): (String, Value)| {
        lua.app_data_mut::<State>()
            .unwrap()
            .global_context
            .insert(name, JValue::from_serialize(value));
        Ok(Value::Nil)
    }))?;
    globals.set("inject", inject)?;

    Ok(Shebang { lua })
}

fn luaize<Arg, T>(
    func: impl Fn(&Lua, Arg) -> crate::Result<T> + 'static,
) -> impl Fn(&Lua, Arg) -> mlua::Result<Value>
where
    Arg: 'static,
    T: IntoLua + 'static,
{
    move |lua, arg| match func(lua, arg)
        .and_then(|x| x.into_lua(lua).map_err(|err| eyre!("{:?}", err)))
    {
        Ok(v) => Ok(v),
        Err(err) => {
            error!("Lua error: {:?}", err);
            Ok(Value::Nil)
        }
    }
}

fn fmt_iso(datetime: impl Into<DateTime<Utc>>) -> String {
    datetime.into().format("%+").to_string()
}
