use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use color_eyre::eyre::eyre;
use minify_js::{Session, TopLevelMode};
use minijinja::{Environment, context, value::merge_maps};

use crate::{
    Result,
    lua::{Render, Shebang as LuaShebang},
    paths, poison,
};

static BUILD_STATUS: AtomicBool = AtomicBool::new(false);

pub fn in_progress() -> bool {
    BUILD_STATUS.load(Ordering::Relaxed)
}

pub fn run() -> Result<()> {
    BUILD_STATUS.store(true, Ordering::Relaxed);
    let result = run_inner();
    BUILD_STATUS.store(false, Ordering::Relaxed);
    result
}

pub fn cleanup() -> Result<()> {
    if !paths::dist()?.exists() {
        return Ok(());
    }

    for child in fs::read_dir(paths::dist()?)? {
        let child = child?.path();

        if child.is_dir() {
            fs::remove_dir_all(child)?;
        } else {
            fs::remove_file(child)?;
        }
    }

    Ok(())
}

fn run_inner() -> Result<()> {
    if !paths::www()?.exists() {
        return Err(eyre!(
            "Please create and populate the www directory: {:?}",
            paths::www()?
        ));
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

    let lua = lua.state();
    let global_context = minijinja::Value::from_serialize(&lua.global_context);
    let merge = |x: &minijinja::Value| merge_maps([global_context.clone(), x.clone()]);

    for name in names {
        let target = paths::dist()?.join(&name);

        if !is_underscored(&target) {
            render(&jinja_env, &name, &target, &merge(&context! {}))?;
        }
    }

    for Render {
        template,
        target,
        context,
    } in &lua.render_queue
    {
        render(&jinja_env, template, target, &merge(context))?;
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

    let context = merge_maps([
        context! {
            __prod => crate::args().prod()
        },
        context.clone(),
    ]);

    let mut data = env.get_template(template)?.render(context)?;
    if !crate::args().antidote {
        data = poison::inject(data)?;
    }

    if let Some("html") = target.extension().and_then(|x| x.to_str()) {
        write_minified(target, Minify::Html(data))?;
    } else {
        fs::write(target, data)?;
    }

    Ok(())
}

enum Minify<T> {
    Html(T),
    Js(T),
}

fn write_minified<T: Into<Vec<u8>>>(target: &Path, data: Minify<T>) -> Result<()> {
    let mut orig_data;

    let data: Result<_> = match data {
        Minify::Html(html) => {
            orig_data = html.into();

            minify_html_onepass::with_friendly_error(
                orig_data.as_mut(),
                &minify_html_onepass::Cfg {
                    minify_css: true,
                    minify_js: true,
                },
            )
            .map_err(|err| eyre!("{:?}: {:?}", target, err))
            .map(|x| orig_data[..x].to_vec())
        }
        Minify::Js(js) => {
            orig_data = js.into();
            let mut data = Vec::new();

            minify_js::minify(
                &mut Session::new(),
                TopLevelMode::Global,
                &orig_data,
                &mut data,
            )
            .map_err(|err| eyre!("{:?}: {:?}", target, err))
            .map(|_| data)
        }
    };

    match data {
        Ok(data) => {
            fs::write(target, data)?;
            Ok(())
        }
        Err(err) => {
            error!("Weirdass error during minification: {:?}", err);
            fs::write(target, orig_data)?;
            Err(err)
        }
    }
}

struct State {
    templates: HashMap<String, String>, // workaround to using owned template sources
    processed_items: HashSet<PathBuf>,
    jinja_env: Environment<'static>,
    lua: LuaShebang,
}

fn walk(branch: &Path, state: &mut State) -> Result<()> {
    let mut result = paths::dist()?.join(branch.strip_prefix(paths::www()?)?);

    if branch.is_dir() {
        let _ = fs::create_dir_all(result);

        let mut ls = Vec::with_capacity(64); // doesn't matter but im GREEDY
        for child in fs::read_dir(branch)? {
            ls.push(child?.path().canonicalize()?);
        }

        ls.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));

        for child in ls {
            walk(&child, state)?;
        }

        return Ok(());
    }

    if state.processed_items.contains(branch) {
        return Ok(());
    }

    state.processed_items.insert(branch.to_path_buf());

    let ext = branch.extension().and_then(OsStr::to_str);
    let underscored = is_underscored(branch);

    match ext {
        Some("j2") => {
            let name = template_name(branch)?;
            let source = fs::read_to_string(branch)?;
            state.templates.insert(name, source);
        }
        Some("scss") if !underscored => {
            let input = fs::read_to_string(branch)?;
            let opts = grass::Options::default().load_path(paths::www()?);
            let data = grass::from_string(input, &opts)?;
            result.set_extension("css");
            fs::write(result, data)?;
        }
        Some("lua") if !underscored => {
            state.lua.process(branch)?;
        }
        Some(ext) if !underscored && !result.exists() => {
            match ext {
                "js" => {
                    let data = fs::read(&branch)?;
                    write_minified(&result, Minify::Js(data))?;
                }
                "html" => {
                    let data = fs::read(&branch)?;
                    write_minified(&result, Minify::Html(data))?;
                }
                _ => {
                    fs::copy(branch, result)?;
                }
            };
        }
        _ => (),
    }

    Ok(())
}

fn is_underscored(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(|x| x.starts_with("_"))
        .unwrap_or(false)
}

fn template_name(path: &Path) -> Result<String> {
    let mut name = String::new();

    let base = path.with_extension("");
    let components = base.strip_prefix(paths::www()?)?.components();

    for comp in components {
        let Component::Normal(x) = comp else {
            continue;
        };

        if !name.is_empty() {
            name += "/";
        }

        let x = x.as_encoded_bytes().iter().cloned().collect();
        name += &String::from_utf8(x)?;
    }

    Ok(name)
}
