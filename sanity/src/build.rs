use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::Instant,
};

use color_eyre::eyre::{self, eyre};
use minijinja::{context, value::merge_maps};
use rayon::prelude::*;
use tokio::sync::{RwLock, RwLockReadGuard};

use crate::{
    fs::PathExt,
    jinja2::JinjaEnvironment,
    lua::Shebang as LuaShebang,
    minify,
    paths::{self, PathExt as _},
};

static BUILD: RwLock<()> = RwLock::const_new(());

pub async fn lock() -> RwLockReadGuard<'static, ()> {
    BUILD.read().await
}

pub async fn run() -> eyre::Result<()> {
    let start = if crate::args().profile_build_times {
        Some(Instant::now())
    } else {
        None
    };

    let result = {
        let _lock = BUILD.write().await;
        run_inner()
    };

    let result = match result {
        Err(err) => {
            error!("{}", err);
            Err(err)
        }
        Ok(()) => {
            info!("Site built!");
            Ok(())
        }
    };

    if let Some(start) = start {
        let duration = Instant::now().duration_since(start);
        info!("Took {}ms", duration.as_millis());
    }

    result
}

fn run_inner() -> eyre::Result<()> {
    if !paths::www()?.exists() {
        return Err(eyre!(
            "Please create and populate the www directory: {}",
            paths::www()?.display_simple()
        ));
    }

    let state = State::try_new()?;
    state.walk(&paths::www()?)?;
    state.finalize()?;

    Ok(())
}

pub fn nuke() {
    match nuke_inner() {
        Ok(()) => info!("Site cleaned!"),
        Err(err) => error!("Clean failed: {}", err),
    }
}

fn nuke_inner() -> eyre::Result<()> {
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

struct StateInner {
    processed_items: HashSet<PathBuf>,
    jinja: JinjaEnvironment,
    lua: LuaShebang,
}

struct State(Mutex<StateInner>);

impl State {
    fn try_new() -> eyre::Result<Self> {
        Ok(Self(Mutex::new(StateInner {
            lua: LuaShebang::try_new()?,
            processed_items: HashSet::new(),
            jinja: JinjaEnvironment::new(),
        })))
    }

    fn lock(&'_ self) -> eyre::Result<MutexGuard<'_, StateInner>> {
        match self.0.lock() {
            Ok(lock) => Ok(lock),
            Err(_) => Err(eyre!("damn it")),
        }
    }

    fn walk(&self, branch: &Path) -> eyre::Result<()> {
        let dest = paths::dist()?.join(branch.strip_prefix(paths::www()?)?);

        if branch.is_dir() {
            let _ = fs::create_dir_all(dest);
            self.process_dir(branch)
        } else if self.lock()?.processed_items.contains(branch) {
            Ok(())
        } else {
            self.lock()?.processed_items.insert(branch.to_path_buf());
            self.process_file(branch, dest)
        }
    }

    fn process_dir(&self, branch: &Path) -> eyre::Result<()> {
        let mut ls = Vec::with_capacity(64); // doesn't matter but im GREEDY

        for child in fs::read_dir(branch)? {
            ls.push(child?.path().canonicalize()?.to_path_buf());
        }

        ls.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
        ls.into_par_iter().try_for_each(|child| self.walk(&child))
    }

    fn process_file(&self, branch: &Path, mut dest: PathBuf) -> eyre::Result<()> {
        let ext = branch.extension_str();
        let underscored = branch.is_underscored();
        let recent = branch.more_recent_than(&dest)?;

        match ext {
            Some("j2") => {
                self.lock()?.jinja.register(branch)?;
            }
            Some("scss") if !underscored => {
                let opts = grass::Options::default().load_path(paths::www()?);
                let data = grass::from_path(branch, &opts)?;
                dest.set_extension("css");
                fs::write(dest, data)?;
            }
            Some("lua") if !underscored => {
                self.lock()?.lua.process(branch)?;
            }
            Some("js") if !recent => {
                let data = fs::read(&branch)?;
                minify::write(&dest, minify::Type::Js, data)?;
            }
            Some("html") if !recent => {
                let data = fs::read(&branch)?;
                minify::write(&dest, minify::Type::Html, data)?;
            }
            _ if !underscored => {
                fs::copy(branch, dest)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn finalize(self) -> eyre::Result<()> {
        let StateInner { jinja, lua, .. } = self.0.into_inner()?;

        let names: HashSet<_> = jinja.all();
        let lua = lua.state();

        let globals = minijinja::Value::from_serialize(&lua.global_context);
        let merge = |x: &minijinja::Value| merge_maps([globals.clone(), x.clone()]);

        names.par_iter().try_for_each(|name| {
            let target = paths::dist()?.join(&name);

            if !target.is_underscored() {
                let ctx = merge(&context! {});
                jinja.render(&name, &target, &ctx)?;
            }

            eyre::Result::<()>::Ok(())
        })?;

        lua.render_queue.par_iter().try_for_each(|r| {
            let ctx = merge(&r.context);
            jinja.render(&r.template, &r.target, &ctx)
        })?;

        Ok(())
    }
}
