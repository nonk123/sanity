use std::net::SocketAddr;

use clap::Parser;
use color_eyre::eyre::eyre;
use http_body_util::Full;
use hyper::{Request, Response, body::Bytes, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

mod build;
mod paths;

#[derive(Parser, Debug)]
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

    let mut args = Args::try_parse()?;
    args.watch |= args.server; // TODO: watch and server...

    if !args.server {
        if args.watch {
            watcher(&args);
        } else {
            return build::run(&args);
        }
    }

    std::thread::spawn(move || watcher(&args));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    let listener = TcpListener::bind(addr).await?;
    println!("Hosting dev-server on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(service))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn service(
    req: Request<hyper::body::Incoming>,
) -> core::result::Result<Response<Full<Bytes>>, color_eyre::Report> {
    let in_path = req.uri().path()[1..].to_string();
    let mut out_path = paths::dist()?.join(in_path);

    if !out_path.exists() {
        return Err(eyre!("Path doesn't exist"));
    }

    if out_path.is_dir() {
        out_path = out_path.join("index.html")
    }

    if !out_path.exists() {
        return Err(eyre!("Path doesn't exist"));
    }

    let data = std::fs::read(out_path)?;
    Ok(Response::new(Full::new(Bytes::from(data))))
}

fn watcher(args: &Args) {
    maybe_build(args);

    loop {
        // TODO: implement...
    }
}

fn maybe_build(args: &Args) {
    if let Err(report) = build::run(args) {
        eprintln!("Failed to (re)build: {:?}", report);
    }
}
