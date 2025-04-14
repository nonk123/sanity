use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};

use color_eyre::eyre::eyre;
use minijinja::{Environment, context};

use crate::{
    Result,
    lua::{Render, Shebang as LuaShebang},
    paths,
};

pub fn run() -> Result<()> {
    if !paths::www()?.exists() {
        return Err(eyre!(
            "Please create and populate the www directory: {:?}",
            paths::www()?
        ));
    }

    if paths::dist()?.exists() {
        for child in fs::read_dir(paths::dist()?)? {
            let child = child?.path();

            if child.is_dir() {
                fs::remove_dir_all(child)?;
            } else {
                fs::remove_file(child)?;
            }
        }
    }

    let mut state = State {
        lua: LuaShebang::try_new()?,
        templates: HashMap::new(),
        processed_items: HashSet::new(),
        jinja_env: Environment::new(),
    };

    walk(&paths::www()?, &mut state)?;

    let State {
        lua,
        templates,
        mut jinja_env,
        ..
    } = state;

    let names: HashSet<_> = templates.keys().map(String::to_string).collect();
    jinja_env.set_loader(move |name| Ok(templates.get(name).map(String::to_string)));

    for name in names {
        let target = paths::dist()?.join(&name);

        if !is_underscored(&target) {
            render(&jinja_env, &name, &target, &context! {})?; // TODO: user-defined context
        }
    }

    let lua_state = lua.state.lock().unwrap();

    for Render {
        template,
        target,
        context,
    } in &lua_state.render_queue
    {
        render(&jinja_env, template, target, context)?;
    }

    Ok(())
}

fn render(
    env: &Environment<'static>,
    template: &str,
    target: &Path,
    context: &minijinja::Value,
) -> Result<()> {
    let parent = target.parent().ok_or(eyre!("No parent directory???"))?;
    let _ = fs::create_dir_all(parent);

    let context = minijinja::value::merge_maps([
        context.clone(),
        context! {
            __prod => crate::args().prod(),
        },
    ]);

    let data = env.get_template(template)?.render(context)?;
    fs::write(target, data)?;

    Ok(())
}

struct State {
    templates: HashMap<String, String>, // workaround to using owned template sources
    processed_items: HashSet<PathBuf>,
    jinja_env: Environment<'static>,
    lua: LuaShebang,
}

fn walk(branch: &Path, state: &mut State) -> Result<()> {
    let mut out_path = paths::dist()?.join(branch.strip_prefix(paths::www()?)?);

    if branch.is_dir() {
        fs::create_dir_all(out_path)?;

        let mut ls = Vec::with_capacity(64); // doesn't matter but im GREEDY

        for child in fs::read_dir(branch)? {
            ls.push(child?.path().canonicalize()?);
        }

        ls.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));

        for child in ls {
            walk(&child, state)?;
        }
    } else if !state.processed_items.contains(branch) {
        state.processed_items.insert(branch.to_path_buf());

        let ext = branch.extension().and_then(OsStr::to_str);
        let underscored = is_underscored(branch);

        match ext {
            Some("j2") => {
                let name = {
                    let mut name = String::new();

                    let base = branch.with_extension("");
                    let components = base.strip_prefix(paths::www()?)?.components();

                    for comp in components {
                        let Component::Normal(x) = comp else {
                            continue;
                        };

                        if !name.is_empty() {
                            name += "/";
                        }

                        name += &String::from_utf8(x.as_encoded_bytes().iter().cloned().collect())?;
                    }

                    name
                };

                let source = fs::read_to_string(branch)?;
                state.templates.insert(name, source);
            }
            Some("scss") if !underscored => {
                let input = fs::read_to_string(branch)?;
                let data = grass::from_string(input, &grass::Options::default())?;

                out_path.set_extension("css");
                fs::write(out_path, data)?;
            }
            Some("lua") if !underscored => {
                state.lua.process(branch)?;
            }
            _ => {
                if !underscored {
                    fs::copy(branch, out_path)?;
                }
            }
        }
    }

    Ok(())
}

fn is_underscored(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(|x| x.starts_with("_"))
        .unwrap_or(false)
}
