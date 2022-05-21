use clap::Parser;
use spurilo::{open, print};
use std::error::Error;

/// The toolbox for parsing and manipulating .GPX files
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// GPX file to use
    #[clap()]
    input: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let info = open(&args.input).await?;

    print(&info)?;

    Ok(())
}
