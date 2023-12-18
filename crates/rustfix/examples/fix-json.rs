#![allow(clippy::print_stderr)]

use std::io::{stdin, BufReader, Read};
use std::{collections::HashMap, collections::HashSet, env, fs};

use anyhow::Error;

fn main() -> Result<(), Error> {
    let suggestions_file = env::args().nth(1).expect("USAGE: fix-json <file or -->");
    let suggestions = if suggestions_file == "--" {
        let mut buffer = String::new();
        BufReader::new(stdin()).read_to_string(&mut buffer)?;
        buffer
    } else {
        fs::read_to_string(&suggestions_file)?
    };
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
        files.entry(file).or_insert_with(Vec::new).push(suggestion);
    }

    for (source_file, suggestions) in &files {
        let source = fs::read_to_string(source_file)?;
        let mut fix = rustfix::CodeFix::new(&source);
        for suggestion in suggestions.iter().rev() {
            if let Err(e) = fix.apply(suggestion) {
                eprintln!("Failed to apply suggestion to {}: {}", source_file, e);
            }
        }
        let fixes = fix.finish()?;
        fs::write(source_file, fixes)?;
    }

    Ok(())
}
