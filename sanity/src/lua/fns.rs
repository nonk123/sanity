use std::fs::{self, File};

use chrono::{DateTime, Utc};
use color_eyre::eyre;
use minijinja::Value as JValue;
use mlua::{Lua, LuaSerdeExt, Value};

use sanity_macros::luafn;

use crate::{
    lua::{LuaFn, Render, State},
    paths,
};

pub fn all() -> Vec<Box<dyn LuaFn + Send>> {
    vec![
        Box::new(render),
        Box::new(json),
        Box::new(read),
        Box::new(lastmod),
        Box::new(inject),
    ]
}

#[luafn]
pub fn render(lua: &Lua, template: String, target: String, context: Value) -> eyre::Result<Value> {
    trace!("lua render: {:?} {:?} => {:?}", template, target, context);

    let mut state = lua.app_data_mut::<State>().unwrap();
    state.render_queue.push(Render {
        context: JValue::from_serialize(context),
        target: paths::dist()?.join(target),
        template,
    });

    Ok(Value::Nil)
}

#[luafn]
pub fn json(lua: &Lua, path: String) -> eyre::Result<Value> {
    let path = paths::www()?.join(path);
    let file = File::open(path)?;
    let serde: JValue = serde_json::from_reader(file)?; // INSANE hack
    Ok(lua.to_value(&serde)?)
}

#[luafn]
pub fn read(lua: &Lua, path: String) -> eyre::Result<String> {
    let path = paths::www()?.join(path);
    Ok(fs::read_to_string(&path)?)
}

#[luafn]
pub fn lastmod(lua: &Lua, path: String) -> eyre::Result<String> {
    let path = paths::www()?.join(path);
    let modif = fs::metadata(&path).and_then(|x| x.modified())?;
    let iso: DateTime<Utc> = modif.into();
    Ok(iso.format("%+").to_string())
}

#[luafn]
pub fn inject(lua: &Lua, name: String, value: Value) -> eyre::Result<Value> {
    lua.app_data_mut::<State>()
        .unwrap()
        .global_context
        .insert(name, JValue::from_serialize(value));
    Ok(Value::Nil)
}
