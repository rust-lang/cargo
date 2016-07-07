#[macro_use]
extern crate quick_error;
extern crate serde_json;
extern crate colored;

extern crate rustfix;

use std::fs::File;
use std::io::{Read, Write};
use colored::Colorize;

fn main() {
    if let Err(error) = try_main() {
        writeln!(std::io::stderr(), "An error occured: {}", error).unwrap();
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), ProgramError> {
    let file_name = try!(std::env::args().skip(1).next().ok_or(ProgramError::NoFile));
    let file = try!(read_file_to_string(&file_name));

    for line in file.lines().filter(not_empty) {
        let deserialized: rustfix::diagnostics::Diagnostic = try!(serde_json::from_str(&line));
        for suggestion in &rustfix::collect_suggestions(&deserialized, None) {
            println!(
                "\n{info}: {message}\n\
                {arrow} {file}:{range}\n\
                {suggestion}\n\n\
                {text}\n\n\
                {with}\n\n\
                {replacement}\n",
                info="Info".green().bold(),
                message=split_at_lint_name(&suggestion.message),
                arrow="-->".blue().bold(),
                suggestion="Suggestion - Replace:".yellow().bold(),
                file=suggestion.file_name, range=suggestion.line_range,
                text=indent(&reset_indent(&suggestion.text)),
                with="with:".yellow().bold(),
                replacement=indent(&suggestion.replacement)
            );
        }
    }

    Ok(())
}

quick_error! {
    /// All possible errors in programm lifecycle
    #[derive(Debug)]
    pub enum ProgramError {
        /// Missing File
        NoFile {
            description("No input file given")
        }
        /// Error while dealing with file or stdin/stdout
        Io(err: std::io::Error) {
            from()
            cause(err)
            description(err.description())
        }
        /// Error with deserialization
        Serde(err: serde_json::Error) {
            from()
            cause(err)
            description(err.description())
        }
    }
}

// Helpers
// -------

fn read_file_to_string(file_name: &str) -> Result<String, std::io::Error> {
    let mut file = try!(File::open(file_name));
    let mut buffer = String::new();
    try!(file.read_to_string(&mut buffer));
    Ok(buffer)
}

fn not_empty(s: &&str) -> bool {
    s.trim().len() > 0
}

fn split_at_lint_name(s: &str) -> String {
    s.split(", #[")
    .collect::<Vec<_>>()
    .join("\n      #[")
}

fn reset_indent(s: &str) -> String {
    let leading_whitespace =
        s.lines()
        .nth(0).unwrap_or("")
        .chars()
        .take_while(|&c| char::is_whitespace(c))
        .count();

    s.lines()
        .map(|line| String::from(&line[leading_whitespace..]))
        .collect::<Vec<_>>()
        .join("\n")
}

fn indent(s: &str) -> String {
    s.lines()
    .map(|l| format!("    {}", l))
    .collect::<Vec<_>>()
    .join("\n")
}
