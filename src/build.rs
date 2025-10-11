use std::{
    collections::HashSet,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use color_eyre::eyre::{self, eyre};
use minijinja::{context, value::merge_maps};

use crate::{
    fsutil::is_underscored, jinja2::JinjaEnvironment, lua::Shebang as LuaShebang, minify, paths,
};

static BUILD_STATUS: AtomicBool = AtomicBool::new(false);

pub fn in_progress() -> bool {
    BUILD_STATUS.load(Ordering::Relaxed)
}

pub fn run() {
    let start = if crate::args().profile_build_times {
        Some(Instant::now())
    } else {
        None
    };

    BUILD_STATUS.store(true, Ordering::Relaxed);
    let result = run_inner();
    BUILD_STATUS.store(false, Ordering::Relaxed);

    match result {
        Ok(()) => info!("Site built!"),
        Err(ref report) => error!("Build failed: {:?}", report),
    }

    if let Some(start) = start {
        let end = Instant::now();
        let duration = end.duration_since(start);
        info!("Took {}ms", duration.as_millis());
    }
}

pub fn nuke() {
    match nuke_inner() {
        Ok(()) => info!("Site cleaned!"),
        Err(err) => error!("Clean failed: {}", err),
    }
}

fn run_inner() -> eyre::Result<()> {
    if !paths::www()?.exists() {
        return Err(eyre!(
            "Please create and populate the www directory: {:?}",
            paths::www()?
        ));
    }

    let mut state = State::new()?;
    state.walk(&paths::www()?)?;
    state.render_html()?;

    Ok(())
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

struct State {
    processed_items: HashSet<PathBuf>,
    jinja: JinjaEnvironment,
    lua: LuaShebang,
}

impl State {
    fn new() -> eyre::Result<Self> {
        Ok(State {
            lua: LuaShebang::try_new()?,
            processed_items: HashSet::new(),
            jinja: JinjaEnvironment::new(),
        })
    }

    fn walk(&mut self, branch: &Path) -> eyre::Result<()> {
        let dest = paths::dist()?.join(branch.strip_prefix(paths::www()?)?);
        if branch.is_dir() {
            let _ = fs::create_dir_all(dest);
            return self.process_dir(branch);
        } else if self.processed_items.contains(branch) {
            return Ok(());
        } else {
            self.processed_items.insert(branch.to_path_buf());
            return self.process_file(branch, dest);
        }
    }

    fn process_dir(&mut self, branch: &Path) -> eyre::Result<()> {
        let mut ls = Vec::with_capacity(64); // doesn't matter but im GREEDY
        for child in fs::read_dir(branch)? {
            ls.push(child?.path().canonicalize()?);
        }

        ls.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));

        for child in ls {
            self.walk(&child)?;
        }

        return Ok(());
    }

    fn process_file(&mut self, branch: &Path, mut dest: PathBuf) -> eyre::Result<()> {
        let ext = branch.extension().and_then(OsStr::to_str);
        let underscored = is_underscored(branch);
        let recent = dest.exists()
            && fs::metadata(&dest)?.modified()? >= fs::metadata(&branch)?.modified()?;

        match ext {
            Some("j2") => {
                self.jinja.register(branch)?;
            }
            Some("scss") if !underscored => {
                let opts = grass::Options::default().load_path(paths::www()?);
                let data = grass::from_path(branch, &opts)?;
                dest.set_extension("css");
                fs::write(dest, data)?;
            }
            Some("lua") if !underscored => {
                self.lua.process(branch)?;
            }
            Some("js") if !underscored && !recent => {
                let data = fs::read(&branch)?;
                minify::write(&dest, minify::Type::Js, data)?;
            }
            Some("html") if !underscored && !recent => {
                let data = fs::read(&branch)?;
                minify::write(&dest, minify::Type::Html, data)?;
            }
            _ => {
                fs::copy(branch, dest)?;
            }
        }

        Ok(())
    }

    fn render_html(&mut self) -> eyre::Result<()> {
        let names: HashSet<_> = self.jinja.all();
        let lua = self.lua.state();

        let globals = minijinja::Value::from_serialize(&lua.global_context);
        let merge = |x: &minijinja::Value| merge_maps([globals.clone(), x.clone()]);

        for name in names {
            let target = paths::dist()?.join(&name);
            if !is_underscored(&target) {
                self.jinja.render(&name, &target, &merge(&context! {}))?;
            }
        }
        for r in &lua.render_queue {
            self.jinja
                .render(&r.template, &r.target, &merge(&r.context))?;
        }

        Ok(())
    }
}
