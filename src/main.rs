#[macro_use]
extern crate log;

use std::{net::SocketAddr, sync::OnceLock, time::Duration};

use clap::Parser;
use color_eyre::eyre::eyre;
use http_body_util::Full;
use hyper::{Request, Response, body::Bytes, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use log::LevelFilter;
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use tokio::net::TcpListener;

mod build;
mod paths;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    watch: bool,
    #[arg(short, long)]
    server: bool,
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

    if !args().server {
        if args().watch {
            return watcher();
        } else {
            return build::run();
        }
    }

    std::thread::spawn(watcher);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    let listener = TcpListener::bind(addr).await?;
    info!("Hosting dev-server on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(http_service))
                .await
            {
                error!("{:?}", err);
            }
        });
    }
}

async fn http_service(
    req: Request<hyper::body::Incoming>,
) -> core::result::Result<Response<Full<Bytes>>, color_eyre::Report> {
    let in_path = req.uri().path()[1..].to_string();
    let mut out_path = paths::dist()?.join(in_path);

    if !out_path.exists() {
        return Err(eyre!("File or directory doesn't exist {:?}", out_path));
    }

    if out_path.is_dir() {
        out_path = out_path.join("index.html")
    }

    if !out_path.exists() {
        return Err(eyre!("File doesn't exist: {:?}", out_path));
    }

    let data = std::fs::read(out_path)?;
    Ok(Response::new(Full::new(Bytes::from(data))))
}

fn watcher() -> Result<()> {
    rebuild();

    let mut debouncer = new_debouncer(
        Duration::from_millis(600),
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
        std::thread::yield_now();
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
