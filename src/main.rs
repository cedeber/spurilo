use clap::{Arg, Command};
use spurilo::{open, print};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("Spurilo: GPX Tools")
        .version("0.1.0-beta.1")
        .author("Cédric Eberhardt <hello+code@cedeber.fr>")
        .about("The toolbox for parsing and manipulating .GPX files")
        .arg(
            Arg::new("INPUT")
                .required(true)
                .help("Sets the input file to use"),
        )
        .get_matches();

    let path = matches.value_of("INPUT").unwrap();
    let info = open(path).await?;

    print(&info)?;

    Ok(())
}
