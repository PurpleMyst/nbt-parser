extern crate nbt_parser;

extern crate failure;

use std::{env, fs};

// TODO: Use `ExitCode` once it gets stabilized.
fn main() -> Result<(), failure::Error> {
    let filename = if let Some(filename) = env::args().nth(1) {
        filename
    } else {
        eprintln!("USAGE: cargo run FILENAME");
        return Ok(());
    };
    let file = fs::File::open(filename)?;
    let nbt_data = nbt_parser::decode(file)?;
    println!("{:#?}", nbt_data);
    Ok(())
}
