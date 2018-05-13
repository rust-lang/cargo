extern crate failure;
extern crate rustfix;

use std::{env, fs, process, collections::HashSet};
use failure::Error;

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let (suggestions_file, source_file) = match args.as_slice() {
        [_, suggestions_file, source_file] => (suggestions_file, source_file),
        _ => {
            println!("USAGE: fix-json <suggestions-file> <source-file>");
            process::exit(1);
        }
    };

    let suggestions = fs::read_to_string(&suggestions_file)?;
    let suggestions = rustfix::get_suggestions_from_json(&suggestions, &HashSet::new())?;

    let source = fs::read_to_string(&source_file)?;

    let fixes = rustfix::apply_suggestions(&source, &suggestions)?;

    println!("{}", fixes);

    Ok(())
}
