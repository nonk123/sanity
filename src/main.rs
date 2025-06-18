#[macro_use]
extern crate log;

use std::{convert::Infallible, fs, net::SocketAddr, sync::OnceLock, thread, time::Duration};

use clap::Parser;
use color_eyre::eyre::eyre;
use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use log::LevelFilter;
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use tokio::net::TcpListener;

mod build;
mod lua;
mod paths;
mod poison;

const DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(1000);

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    watch: bool,
    #[arg(short, long)]
    server: bool,
    #[arg(short, long)]
    force_prod: bool,
    #[arg(short, long)]
    antidote: bool,
    #[arg(short, long)]
    lualib: bool,
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

    let mut args0 = Args::try_parse()?;
    args0.watch |= args0.server;
    ARGS.set(args0).unwrap();

    if args().lualib {
        std::fs::write(paths::root()?.join("_sanity.lua"), include_str!("lib.lua"))?;
    }

    if !args().server {
        if args().watch {
            return watcher();
        } else {
            return build::run();
        }
    }

    thread::spawn(watcher);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    let listener = TcpListener::bind(addr).await?;
    info!("Hosting dev-server on http://{}", addr);

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
    while build::in_progress() {
        thread::yield_now();
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

    let data = fs::read(out_path)?;
    Ok(Response::new(Full::new(Bytes::from(data))))
}

fn watcher() -> Result<()> {
    rebuild();

    let mut debouncer =
        new_debouncer(
            DEBOUNCE_TIMEOUT,
            None,
            |result: DebounceEventResult| match result {
                Ok(events) => {
                    for event in events {
                        if !event.kind.is_access() {
                            rebuild();
                            break;
                        }
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
        thread::sleep(Duration::from_millis(100));
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
