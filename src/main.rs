use clap::{App, Arg};
use spurilo::open;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("Sprurilo: GPX Tools")
        .version("0.1.0-beta.1")
        .author("Cédric Eberhardt <hello+code@cedeber.fr>")
        .about("The toolbox for parsing and manipulating .GPX files")
        .arg(
            Arg::new("INPUT")
                .required(true)
                .about("Sets the input file to use"),
        )
        .get_matches();

    let path = matches.value_of("INPUT").unwrap();

    open(path).await?;

    Ok(())
}
