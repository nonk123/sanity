#[macro_use]
extern crate log;

use std::{
    collections::HashSet, convert::Infallible, ffi::OsStr, fs, net::SocketAddr, sync::OnceLock,
    thread, time::Duration,
};

use clap::Parser;
use color_eyre::eyre::eyre;
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
use notify_debouncer_full::{DebounceEventResult, DebouncedEvent, new_debouncer};
use tokio::net::TcpListener;

mod build;
mod lua;
mod paths;
mod poison;

const DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(1000);

/// The only sane static site generator in existence.
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Run a filesystem watcher which rebuilds the project on start and on changes in `www`. Otherwise, just build the whole project once.
    #[arg(short, long)]
    watch: bool,
    /// Run an HTTP dev-server with a filesystem watcher on http://localhost:8000 (or a different port, if `--port` is specified).
    #[arg(short, long)]
    server: bool,
    /// Force-set `__prod` to `true` in all templates.
    ///
    /// Use `__prod` inside templates to remove markup which is considered useless on a local dev-server.
    ///
    /// Refer to README.md for usage.
    /// ```
    #[arg(short, long)]
    force_prod: bool,
    /// Bypass LLM poisoning if this feature is enabled at compile-time.
    #[arg(short, long)]
    antidote: bool,
    /// Write Lua function definitions to disk.
    ///
    /// To use them in VS Code, add the following to `settings.json`: `"Lua.workspace.library": ["_sanity.lua"]`.
    #[arg(short, long)]
    lualib: bool,
    /// Set the listening port for the dev-server.
    #[arg(short, long, default_value_t = 8000)]
    port: u16,
}

impl Args {
    pub fn prod(&self) -> bool {
        self.force_prod || (!self.server && !self.watch)
    }
}

pub type Result<T> = color_eyre::eyre::Result<T>;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = color_eyre::install();

    pretty_env_logger::formatted_builder()
        .filter_level(LevelFilter::Info)
        .try_init()?;

    let mut args0 = Args::parse();
    args0.watch |= args0.server;
    ARGS.set(args0).unwrap();

    if args().lualib {
        fs::write(paths::root()?.join("_sanity.lua"), include_str!("lib.lua"))?;
    }

    build::cleanup()?;
    if !args().server {
        if args().watch {
            return watcher();
        } else {
            return build::run();
        }
    }

    thread::spawn(watcher);

    let addr = SocketAddr::from(([127, 0, 0, 1], args().port));
    let listener = TcpListener::bind(addr).await?;
    info!("Hosting dev-server on http://localhost:{}", args().port);

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

fn waste_cycles() {
    thread::sleep(Duration::from_millis(100));
}

async fn http_service(
    req: Request<Incoming>,
) -> core::result::Result<Response<Full<Bytes>>, Infallible> {
    while build::in_progress() {
        waste_cycles();
    }

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

fn _http_service(req: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
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

fn process_events(events: Vec<DebouncedEvent>) -> Result<()> {
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
        rebuild();
    }

    Ok(())
}

fn watcher() -> Result<()> {
    rebuild();

    let mut debouncer =
        new_debouncer(
            DEBOUNCE_TIMEOUT,
            None,
            |result: DebounceEventResult| match result {
                Ok(events) => {
                    if let Err(error) = process_events(events) {
                        error!("{:?}", error);
                    }
                }
                Err(errors) => {
                    for error in errors {
                        error!("{:?}", error);
                    }
                }
            },
        )?;

    debouncer.watch(&paths::www()?, RecursiveMode::Recursive)?;
    info!("Watching {:?}", paths::www()?);

    loop {
        waste_cycles();
    }
}

fn rebuild() {
    match build::run() {
        Ok(()) => info!("Rebuilt!"),
        Err(report) => error!("Failed to rebuild: {:?}", report),
    }
}

static ARGS: OnceLock<Args> = OnceLock::new();

pub fn args() -> &'static Args {
    ARGS.get().unwrap()
}
