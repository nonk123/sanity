use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use color_eyre::eyre::eyre;
use minijinja::{Environment, context};

use crate::{
    lua::{Render, Shebang},
    paths,
};

// Limited to a single instance because the closures moving an `Arc` would cause memory leaks on repeated usage.
static LUA_SHEBANG: OnceLock<Shebang> = OnceLock::new();

pub fn preflight() -> crate::Result<()> {
    LUA_SHEBANG
        .set(crate::lua::new()?)
        .or(Err(eyre!("Failed to initialize the Lua shebang")))?;
    Ok(())
}

fn postbuild_cleanup() {
    LUA_SHEBANG.get().unwrap().postbuild_cleanup();
}

fn _run() -> crate::Result<()> {
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
        templates: HashMap::new(),
        read_paths: HashSet::new(),
        env: Environment::new(),
    };

    walk(&paths::www()?, &mut state)?;

    let State {
        templates, mut env, ..
    } = state;

    let names: HashSet<String> = templates.keys().map(|x| x.to_string()).collect();
    env.set_loader(move |name| Ok(templates.get(name).map(|x| x.to_string())));

    for name in names {
        let out_path = paths::dist()?.join(&name);

        if !is_underscored(&out_path) {
            let data = env.get_template(&name)?.render(context! {})?; // TODO: user-defined context
            fs::write(out_path, data)?;
        }
    }

    let state = LUA_SHEBANG.get().unwrap().state.lock().unwrap();

    for Render {
        template,
        target,
        context,
    } in &state.render_queue
    {
        let data = env.get_template(template)?.render(context.clone())?;
        fs::write(target, data)?;
    }

    Ok(())
}

pub fn run() -> crate::Result<()> {
    let result = _run();
    postbuild_cleanup();
    result
}

struct State {
    templates: HashMap<String, String>, // workaround to using owned template sources
    read_paths: HashSet<PathBuf>,
    env: Environment<'static>,
}

fn walk(in_path: &Path, state: &mut State) -> crate::Result<()> {
    let mut out_path = paths::dist()?.join(in_path.strip_prefix(paths::www()?)?);

    if in_path.is_dir() {
        fs::create_dir_all(out_path)?;

        let mut ls = Vec::new();

        for child in fs::read_dir(in_path)? {
            ls.push(child?.path());
        }

        ls.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));

        for child in ls {
            walk(&child, state)?;
        }
    } else if !state.read_paths.contains(in_path) {
        let ext = in_path.extension().and_then(|x| x.to_str());

        match ext {
            Some("j2") => {
                state.read_paths.insert(in_path.to_path_buf());

                let name = out_path
                    .with_extension("")
                    .strip_prefix(paths::dist()?)?
                    .to_str()
                    .ok_or_else(|| eyre!("File names should be UTF-8"))?
                    .to_string();

                let source = fs::read_to_string(in_path)?;
                state.templates.insert(name, source);
            }
            Some("scss") if !is_underscored(in_path) => {
                state.read_paths.insert(in_path.to_path_buf());

                let input = fs::read_to_string(in_path)?;
                let data = grass::from_string(input, &grass::Options::default())?;

                out_path.set_extension("css");
                fs::write(out_path, data)?;
            }
            Some("lua") if !is_underscored(in_path) => {
                state.read_paths.insert(in_path.to_path_buf());
                LUA_SHEBANG.get().unwrap().process(in_path)?;
            }
            _ if !is_underscored(in_path) => {
                state.read_paths.insert(in_path.to_path_buf());
                fs::copy(in_path, out_path)?;
            }
            _ => (),
        }
    }

    Ok(())
}

fn is_underscored(path: &Path) -> bool {
    path.file_name()
        .and_then(|x| x.to_str())
        .map(|x| x.starts_with("_"))
        .unwrap_or(false)
}
