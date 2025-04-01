use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use color_eyre::eyre::eyre;
use minijinja::{Environment, context};

use crate::Args;

pub fn root() -> crate::Result<PathBuf> {
    Ok(std::env::current_dir()?.canonicalize()?)
}

pub fn www() -> crate::Result<PathBuf> {
    Ok(root()?.join("www"))
}

pub fn dist() -> crate::Result<PathBuf> {
    Ok(root()?.join("dist"))
}

pub fn run(_: &Args) -> crate::Result<()> {
    if !www()?.exists() {
        return Err(eyre!(
            "Please create and populate the www directory {:?}",
            www()?
        ));
    }

    if dist()?.exists() {
        for child in fs::read_dir(dist()?)? {
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
        env: Environment::new(),
    };

    walk(&www()?, &mut state)?;

    let State { templates, mut env } = state;

    let templates0 = templates.clone();
    env.set_loader(move |name| Ok(templates0.get(name).map(|x| x.to_string())));

    for (name, _) in templates {
        let out_path = dist()?.join(&name);

        if is_underscored(&out_path) {
            continue;
        }

        let data = env.get_template(&name)?.render(context! {})?; // TODO: user-defined context
        fs::write(out_path, data)?;
    }

    Ok(())
}

struct State<'a> {
    templates: HashMap<String, String>,
    env: Environment<'a>,
}

fn walk(in_path: &Path, state: &mut State) -> crate::Result<()> {
    let mut out_path = dist()?.join(in_path.strip_prefix(www()?)?);

    if in_path.is_dir() {
        fs::create_dir_all(out_path)?;

        for child in fs::read_dir(in_path)? {
            walk(&child?.path(), state)?;
        }
    } else {
        let ext = in_path.extension().and_then(|x| x.to_str());

        match ext {
            Some("j2") => {
                out_path.set_extension("");

                let name = out_path
                    .strip_prefix(dist()?)?
                    .to_str()
                    .ok_or_else(|| eyre!("Files should have a basename"))?
                    .to_string();

                let source = fs::read_to_string(in_path)?;
                state.templates.insert(name, source);
            }
            Some("scss") if !is_underscored(in_path) => {
                let input = fs::read_to_string(in_path)?;
                let data = grass::from_string(input, &grass::Options::default())?;

                out_path.set_extension("css");
                fs::write(out_path, data)?;
            }
            _ if !is_underscored(in_path) => {
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
