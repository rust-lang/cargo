#[macro_use]
extern crate quick_error;
extern crate serde_json;
extern crate colored;

extern crate rustfix;

use std::fs::File;
use std::io::{Read, Write};
use colored::Colorize;

const USER_OPTIONS: &'static str = "What do you want to do? \
    [r]eplace | [s]kip | save and [q]uit | [a]bort (without saving)";

fn main() {
    let program = try_main();
    match program {
        Ok(_) => std::process::exit(0),
        Err(ProgramError::UserAbort) => {
            writeln!(std::io::stdout(), "{}", ProgramError::UserAbort).unwrap();
            std::process::exit(0);
        }
        Err(error) => {
            writeln!(std::io::stderr(), "An error occured: {:#?}", error).unwrap();
            std::process::exit(1);
        }
    }
}

macro_rules! flush {
    () => (try!(std::io::stdout().flush());)
}

fn try_main() -> Result<(), ProgramError> {
    let file_name = try!(std::env::args().skip(1).next().ok_or(ProgramError::NoFile));
    let file = try!(read_file_to_string(&file_name));

    let mut accepted_suggestions: Vec<rustfix::Suggestion> = vec![];

    'diagnostics: for line in file.lines().filter(not_empty) {
        let deserialized: rustfix::diagnostics::Diagnostic = try!(serde_json::from_str(&line));
        'suggestions: for suggestion in &rustfix::collect_suggestions(&deserialized, None) {
            println!("\n\n{info}: {message}\n{arrow} \
                      {file}:{range}\n{suggestion}\n\n{text}\n\n{with}\n\n{replacement}\n",
                     info = "Info".green().bold(),
                     message = split_at_lint_name(&suggestion.message),
                     arrow = "  -->".blue().bold(),
                     suggestion = "Suggestion - Replace:".yellow().bold(),
                     file = suggestion.file_name,
                     range = suggestion.line_range,
                     text = indent(4, &reset_indent(&suggestion.text)),
                     with = "with:".yellow().bold(),
                     replacement = indent(4, &suggestion.replacement));

            'userinput: loop {
                print!("{arrow} {user_options}\n\
                    {prompt} ",
                       arrow = "==>".green().bold(),
                       prompt = "  >".green().bold(),
                       user_options = USER_OPTIONS.green());

                flush!();
                let mut input = String::new();
                try!(std::io::stdin().read_line(&mut input));

                match input.trim() {
                    "s" => {
                        println!("Skipped.");
                        continue 'suggestions;
                    }
                    "r" => {
                        accepted_suggestions.push((*suggestion).clone());
                        println!("Suggestion accepted. I'll remember that and apply it later.");
                        continue 'suggestions;
                    }
                    "q" => {
                        println!("Thanks for playing!");
                        break 'diagnostics;
                    }
                    "a" => {
                        return Err(ProgramError::UserAbort);
                    }
                    _ => {
                        println!("{error}: I didn't quite get that. {user_options}",
                                 error = "Error".red().bold(),
                                 user_options = USER_OPTIONS);
                        continue 'userinput;
                    }
                }
            }
        }

    }

    if !accepted_suggestions.is_empty() {
        println!("Good work. Let me just apply these {} changes!",
                 accepted_suggestions.len());

        for suggestion in accepted_suggestions.iter().rev() {
            try!(apply_suggestion(suggestion));

            print!(".");
            flush!();
        }

        println!("\nDone.");
    }

    println!("See you around!");

    Ok(())
}

quick_error! {
    /// All possible errors in programm lifecycle
    #[derive(Debug)]
    pub enum ProgramError {
        UserAbort {
            description("Let's get outta here!")
        }
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
    !s.trim().is_empty()
}

fn split_at_lint_name(s: &str) -> String {
    s.split(", #[")
        .collect::<Vec<_>>()
        .join("\n      #[") // Length of whitespace == length of "Info: "
}

fn reset_indent(s: &str) -> String {
    let leading_whitespace = s.lines()
        .nth(0)
        .unwrap_or("")
        .chars()
        .take_while(|&c| char::is_whitespace(c))
        .count();

    s.lines()
        .map(|line| String::from(&line[leading_whitespace..]))
        .collect::<Vec<_>>()
        .join("\n")
}

fn indent(size: u32, s: &str) -> String {
    let whitespace: String = std::iter::repeat(' ').take(size as usize).collect();

    s.lines()
        .map(|l| format!("{}{}", whitespace, l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Apply suggestion to a file
///
/// Please beware of ugly hacks below! Originally, I wanted to replace byte ranges, but sadly the
/// ranges rustc's JSON output gives me do not correspond to the parts of the file they are meant
/// to correspond to. So, for now, let's just replace lines!
///
/// This function is as stupid as possible. Make sure you call for the replacemnts in one file in
/// reverse order to not mess up the lines for replacements further down the road.
fn apply_suggestion(suggestion: &rustfix::Suggestion) -> Result<(), ProgramError> {
    use std::cmp::max;
    use std::iter::repeat;

    let file_content = try!(read_file_to_string(&suggestion.file_name));
    let mut new_content = String::new();

    // Add the lines before the section we want to replace
    new_content.push_str(&file_content.lines()
        .take(max(suggestion.line_range.start.line - 1, 0) as usize)
        .collect::<Vec<_>>()
        .join("\n"));
    new_content.push_str("\n");

    // Indentation
    new_content.push_str(&repeat(" ")
        .take(suggestion.line_range.start.column - 1 as usize)
        .collect::<String>());

    // TODO(killercup): Replace sections of lines only
    new_content.push_str(suggestion.replacement.trim());

    // Add the lines after the section we want to replace
    new_content.push_str("\n");
    new_content.push_str(&file_content.lines()
        .skip(suggestion.line_range.end.line as usize)
        .collect::<Vec<_>>()
        .join("\n"));
    new_content.push_str("\n");

    let mut file = try!(File::create(&suggestion.file_name));
    let new_content = new_content.as_bytes();

    try!(file.set_len(new_content.len() as u64));
    try!(file.write_all(&new_content));

    Ok(())
}
