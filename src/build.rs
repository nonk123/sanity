use std::{collections::HashMap, fs, path::Path};

use color_eyre::eyre::eyre;
use minijinja::{Environment, context};

use crate::paths;

pub fn run() -> crate::Result<()> {
    if !paths::www()?.exists() {
        return Err(eyre!(
            "Please create and populate the www directory {:?}",
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
        env: Environment::new(),
    };

    walk(&paths::www()?, &mut state)?;

    let State { templates, mut env } = state;

    let templates0 = templates.clone();
    env.set_loader(move |name| Ok(templates0.get(name).map(|x| x.to_string())));

    for (name, _) in templates {
        let out_path = paths::dist()?.join(&name);

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
    let mut out_path = paths::dist()?.join(in_path.strip_prefix(paths::www()?)?);

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
                    .strip_prefix(paths::dist()?)?
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
