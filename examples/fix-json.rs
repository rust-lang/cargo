extern crate failure;
extern crate rustfix;

use failure::Error;
use std::{collections::HashMap, collections::HashSet, env, fs, process};

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let suggestions_file = match args.as_slice() {
        [_, suggestions_file] => suggestions_file,
        _ => {
            println!("USAGE: fix-json <suggestions-file>");
            process::exit(1);
        }
    };

    let suggestions = fs::read_to_string(&suggestions_file)?;
    let suggestions = rustfix::get_suggestions_from_json(
        &suggestions,
        &HashSet::new(),
        rustfix::Filter::Everything,
    )?;

    let mut files = HashMap::new();
    for suggestion in suggestions {
        let file = suggestion.solutions[0].replacements[0]
            .snippet
            .file_name
            .clone();
        let entry = files.entry(file).or_insert(Vec::new());
        entry.push(suggestion);
    }

    for (source_file, suggestions) in &files {
        let source = fs::read_to_string(&source_file)?;
        let fixes = rustfix::apply_suggestions(&source, suggestions)?;
        fs::write(&source_file, fixes)?;
    }

    Ok(())
}
