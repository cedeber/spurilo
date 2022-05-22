use clap::Parser;
use spurilo::{draw, open, parse, print};
use std::error::Error;

/// The toolbox for parsing and manipulating .GPX files
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// GPX file to use
    #[clap()]
    input: String,

    /// Export as image (BETA)
    #[clap(long)]
    draw: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if let Ok(gpx) = open(&args.input).await {
        if let Ok((info, line)) = parse(&gpx).await {
            print(&info)?;

            if args.draw {
                draw(&line, &info).await?;
            }
        }
    }

    Ok(())
}
