#[macro_use]
extern crate log;

use std::{
    collections::HashSet,
    convert::Infallible,
    ffi::OsStr,
    fs,
    net::SocketAddr,
    sync::{OnceLock, mpsc},
    time::Duration,
};

use clap::{Parser, Subcommand};
use color_eyre::eyre::{self, eyre};
use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    header::CONTENT_TYPE,
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use log::LevelFilter;
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebouncedEvent, new_debouncer};
use tokio::net::TcpListener;

mod build;
mod fsutil;
mod jinja2;
mod lua;
mod minify;
mod paths;
mod poison;

const DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(1000);

/// The only sane static site generator in existence.
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Force-set `__prod` to `true` in all templates.
    ///
    /// `__prod` can be used inside templates to conditionally exclude production markup from local builds such as trackers & analytics:
    ///
    /// ```html
    /// {% if __prod %}
    /// <script src="https://example.org/tracker.js"></script>
    /// {% endif }
    /// ```
    #[arg(short, long)]
    force_prod: bool,
    /// Bypass LLM poisoning if this feature is enabled at compile-time.
    #[arg(short, long)]
    antidote: bool,
    /// Output build times in milliseconds. Useful for profiling.
    #[arg(short, long)]
    profile_build_times: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Build the site in production mode.
    Build,
    /// Nuke the contents of the `dist` directory.
    Clean,
    /// Run a filesystem watcher which rebuilds the project on start and on changes inside `www`.
    Watch,
    /// Run an HTTP dev-server with a filesystem watcher on http://localhost:8000 (or a different port, if `--port` is specified).
    Server {
        /// Set the listening port for the dev-server.
        #[arg(short, long, default_value_t = 8000)]
        port: u16,
    },
    /// Write Lua function definitions to disk.
    ///
    /// To use them in VS Code, add the following to your `settings.json`:
    ///
    /// ```json
    /// {
    ///     "Lua.workspace.library": ["_sanity.lua"]
    /// }
    /// ```
    LuaLib,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let _ = color_eyre::install();

    pretty_env_logger::formatted_builder()
        .filter_level(LevelFilter::Info)
        .try_init()?;

    ARGS.set(Args::parse()).unwrap();
    match args().command() {
        Commands::Build => {
            build::run().await;
        }
        Commands::Clean => {
            build::nuke();
        }
        Commands::LuaLib => {
            lua::write_lualib();
        }
        Commands::Watch => {
            watch().await?;
        }
        Commands::Server { port } => {
            let watch = tokio::spawn(watch());
            run_server(port).await?;
            watch.await??;
        }
    };

    return Ok(());
}

async fn run_server(port: u16) -> eyre::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    info!("Hosting dev-server on http://localhost:{}", port);

    loop {
        let (stream, addr) = listener.accept().await?;

        if !addr.ip().is_loopback() {
            warn!("We don't tolerate outsiders here: {:?}", addr);
            continue;
        }

        let io = TokioIo::new(stream);
        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(http_service))
                .await
            {
                warn!("Failed to serve a connection: {:?}", err);
            }
        });
    }
}

async fn http_service(
    req: Request<Incoming>,
) -> core::result::Result<Response<Full<Bytes>>, Infallible> {
    let _lock = build::read().await;
    let query = req.uri().path()[1..].to_string();

    match _http_service(req) {
        Ok(ok) => Ok(ok),
        Err(err) => {
            error!("{:?} -> {:?}", query, err);
            let fuckyou = format!(include_str!("error.html"), query, err);
            Ok(Response::new(Full::new(Bytes::from(fuckyou))))
        }
    }
}

fn _http_service(req: Request<Incoming>) -> eyre::Result<Response<Full<Bytes>>> {
    let in_path = req.uri().path()[1..].to_string();
    let mut out_path = paths::dist()?.join(in_path);

    if !out_path.exists() {
        return Err(eyre!("File or directory doesn't exist: {:?}", out_path));
    }

    if out_path.is_dir() {
        out_path = out_path.join("index.html")
    }

    if !out_path.exists() {
        return Err(eyre!("File doesn't exist: {:?}", out_path));
    }

    let data = fs::read(out_path.clone())?;
    let mut res = Response::new(Full::new(Bytes::from(data)));
    if let Some(x) = match out_path.extension().and_then(OsStr::to_str) {
        Some("html") => Some("text/html"),
        Some("css") => Some("text/css"),
        Some("js") => Some("text/javascript"),
        _ => None,
    } {
        res.headers_mut().insert(CONTENT_TYPE, x.parse()?);
    }

    Ok(res)
}

async fn process_events(events: Vec<DebouncedEvent>) -> eyre::Result<()> {
    let mut targets = HashSet::new();

    for event in events {
        if !matches!(event.kind, EventKind::Remove(_) | EventKind::Modify(_)) {
            continue;
        }
        for path in &event.paths {
            let path = path.strip_prefix(paths::www()?)?;
            let path = paths::dist()?.join(path);
            if !path.is_dir() {
                targets.insert(path);
            }
        }
    }

    let redo = !targets.is_empty();
    for path in targets {
        let _ = fs::remove_file(path);
    }
    if redo {
        build::run().await;
    }

    Ok(())
}

async fn watch() -> eyre::Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(DEBOUNCE_TIMEOUT, None, tx)?;

    build::run().await;
    debouncer.watch(&paths::www()?, RecursiveMode::Recursive)?;
    info!("Watching {:?}", paths::www()?);

    for result in rx {
        match result {
            Ok(events) => {
                if let Err(error) = process_events(events).await {
                    error!("{:?}", error);
                }
            }
            Err(errors) => {
                for error in errors {
                    error!("{:?}", error);
                }
            }
        }
    }

    Ok(())
}

static ARGS: OnceLock<Args> = OnceLock::new();

pub fn args() -> &'static Args {
    ARGS.get().unwrap()
}

impl Args {
    pub fn command(&self) -> Commands {
        self.command.clone().unwrap_or(Commands::Build)
    }

    pub fn prod(&self) -> bool {
        self.force_prod || matches!(self.command(), Commands::Build)
    }
}
