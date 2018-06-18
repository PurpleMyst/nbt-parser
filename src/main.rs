extern crate combine;
extern crate nbt_parser;

use combine::Parser;

fn main() {
    let filename = std::env::args().nth(1).expect("USAGE: cargo run FILENAME");
    let contents = std::fs::read(filename).unwrap();
    let mut parser = nbt_parser::named_tag();
    let nbt_data = parser.easy_parse(contents.as_slice()).unwrap().0;
    println!("{:#?}", nbt_data);
}
