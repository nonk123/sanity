use clap::Parser;

mod build;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    watch: bool,
    #[arg(short, long)]
    server: bool,
}

pub type Result<T> = color_eyre::eyre::Result<T>;

fn main() -> Result<()> {
    let _ = color_eyre::install();

    let mut args = Args::try_parse()?;
    args.watch |= args.server; // TODO: watch and server...

    build::run(&args)?;

    Ok(())
}
