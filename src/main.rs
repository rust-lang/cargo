#[macro_use]
extern crate quick_error;
extern crate serde_json;
extern crate colored;

extern crate rustfix;

use std::fs::File;
use std::io::{Read, Write};
use colored::Colorize;

const USER_OPTIONS: &'static str = "What do you want to do? \
    [r]eplace/[s]kip/save and [q]uit/[a]bort (without saving)";

fn main() {
    if let Err(error) = try_main() {
        writeln!(std::io::stderr(), "An error occured: {:#?}", error).unwrap();
        std::process::exit(1);
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
            println!(
                "\n{info}: {message}\n\
                {arrow} {file}:{range}\n\
                {suggestion}\n\n\
                {text}\n\n\
                {with}\n\n\
                {replacement}\n",
                info="Info".green().bold(),
                message=split_at_lint_name(&suggestion.message),
                arrow="  -->".blue().bold(),
                suggestion="Suggestion - Replace:".yellow().bold(),
                file=suggestion.file_name, range=suggestion.line_range,
                text=indent(4, &reset_indent(&suggestion.text)),
                with="with:".yellow().bold(),
                replacement=indent(4, &suggestion.replacement)
            );

            'userinput: loop {
                print!("{arrow} {user_options}\n\
                    {prompt} ",
                    arrow="==>".green().bold(),
                    prompt="  >".green().bold(),
                    user_options=USER_OPTIONS.green());

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
                    },
                    "q" => {
                        println!("Good work. Let me just apply these {} changes!",
                            accepted_suggestions.len());

                        try!(apply_suggestions(&accepted_suggestions));

                        println!("\nDone.");
                        println!("Thanks for playing!");
                        break 'diagnostics;
                    }
                    "a" => {
                        println!("Let's get outta here!");
                        break 'diagnostics;
                    }
                    _ => {
                        println!("{error}: I didn't quite get that. {user_options}",
                            error="Error".red().bold(), user_options=USER_OPTIONS);
                        continue 'userinput;
                    }
                }
            }
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
    !s.trim().is_empty()
}

fn split_at_lint_name(s: &str) -> String {
    s.split(", #[")
    .collect::<Vec<_>>()
    .join("\n      #[") // Length of whitespace == length of "Info: "
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

fn indent(size: u32, s: &str) -> String {
    let whitespace: String = std::iter::repeat(' ').take(size as usize).collect();

    s.lines()
    .map(|l| format!("{}{}", whitespace, l))
    .collect::<Vec<_>>()
    .join("\n")
}

fn apply_suggestions(suggestions: &[rustfix::Suggestion]) -> Result<(), ProgramError> {
    for suggestion in suggestions.iter().rev() {
        // let file_content = read_file_to_string(&suggestion.file_name);
        // file_content.lines().skip(suggestion.line_range.0.0)
        //     .take(suggestion.line_range.1.0) - suggestion.line_range.0.0;

        let mut file = try!(File::open(&suggestion.file_name));
        let mut file_content = vec![];
        try!(file.read_to_end(&mut file_content));

        let mut new_content = vec![];
        new_content.extend_from_slice(&file_content[..suggestion.byte_range.0]);
        new_content.extend_from_slice(suggestion.replacement.as_bytes());
        new_content.extend_from_slice(&file_content[suggestion.byte_range.1..]);

        let mut file = try!(File::create(&suggestion.file_name));

        try!(file.set_len(new_content.len() as u64));
        try!(file.write_all(&new_content));

        print!(".");
        flush!();
    }

    Ok(())
}
