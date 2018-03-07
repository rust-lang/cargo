#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate clap;
extern crate colored;

extern crate rustfix;

use std::fs::File;
use std::io::{Read, Write};
use std::error::Error;
use std::process::Command;
use std::collections::HashSet;

use colored::Colorize;
use clap::{Arg, App};

use std::str::FromStr;

use rustfix::{Suggestion, Replacement};
use rustfix::diagnostics::Diagnostic;

const USER_OPTIONS: &str = "What do you want to do? \
    [0-9] | [r]eplace | [s]kip | save and [q]uit | [a]bort (without saving)";

fn main() {
    let program = try_main();
    match program {
        Ok(_) => std::process::exit(0),
        Err(ProgramError::UserAbort) => {
            writeln!(std::io::stdout(), "{}", ProgramError::UserAbort).unwrap();
            std::process::exit(0);
        }
        Err(error) => {
            writeln!(std::io::stderr(), "An error occured: {}", error).unwrap();
            writeln!(std::io::stderr(), "{:?}", error).unwrap();
            if let Some(cause) = error.cause() {
                writeln!(std::io::stderr(), "Cause: {:?}", cause).unwrap();
            }
            std::process::exit(1);
        }
    }
}

macro_rules! flush {
    () => (try!(std::io::stdout().flush());)
}

/// A list of `--only` aliases
const ALIASES: &[(&str, &[&str])] = &[
    ("use", &["E0412"]),
];

fn try_main() -> Result<(), ProgramError> {
    let matches = App::new("rustfix")
        .about("Automatically apply suggestions made by rustc")
        .version(crate_version!())
        .arg(Arg::with_name("clippy")
            .long("clippy")
            .help("Use `cargo clippy` for suggestions"))
        .arg(Arg::with_name("yolo")
            .long("yolo")
            .help("Automatically apply all unambiguous suggestions"))
        .arg(Arg::with_name("only")
            .long("only")
            .help("Only show errors or lints with the specific id(s) (comma separated)")
            .use_delimiter(true))
        .arg(Arg::with_name("file")
            .long("file")
            .short("f")
            .takes_value(true)
            .help("Load errors from the given JSON file (produced by `cargo build --message-format=json`)"))
        .get_matches();

    let mut extra_args = Vec::new();

    if !matches.is_present("clippy") {
        extra_args.push("-Aclippy");
    }

    let mode = if matches.is_present("yolo") {
        AutofixMode::Yolo
    } else {
        AutofixMode::None
    };

    let mut only: HashSet<String> = matches
        .values_of("only")
        .map_or(HashSet::new(), |values| {
            values.map(ToString::to_string).collect()
        });

    for alias in ALIASES {
        if only.remove(alias.0) {
            for alias in alias.1 {
                only.insert(alias.to_string());
            }
        }
    }

    // Get JSON output from rustc...
    let json = if let Some(file) = matches.value_of("file") {
        let mut f = File::open(file)?;
        let mut j = "".into();
        f.read_to_string(&mut j)?;
        j
    } else {
        get_json(&extra_args, matches.is_present("clippy"))?
    };

    let suggestions: Vec<Suggestion> = json.lines()
        .filter(not_empty)
        // Convert JSON string (and eat parsing errors)
        .flat_map(|line| serde_json::from_str::<CargoMessage>(line))
        // One diagnostic line might have multiple suggestions
        .filter_map(|cargo_msg| rustfix::collect_suggestions(&cargo_msg.message, &only))
        .collect();

    try!(handle_suggestions(suggestions, mode));

    Ok(())
}

#[derive(Deserialize)]
struct CargoMessage {
    message: Diagnostic,
}

fn get_json(extra_args: &[&str], clippy: bool) -> Result<String, ProgramError> {
    let build_cmd = if clippy {
        "clippy"
    } else {
        "rustc"
    };
    let output = try!(Command::new("cargo")
        .args(&[build_cmd, "--message-format", "json"])
        .arg("--")
        .args(extra_args)
        .output());

    Ok(String::from_utf8(output.stdout)?)
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum AutofixMode {
    /// Do not apply any fixes automatically
    None,
    // /// Only apply suggestions of a whitelist of lints
    // Whitelist,
    // /// Check the confidence flag supplied by rustc
    // Confidence,
    /// Automatically apply all unambiguous suggestions
    Yolo,
}

fn prelude(suggestion: &Replacement) {
    let snippet = &suggestion.snippet;
    if snippet.text.1.is_empty() {
        // Check whether this suggestion wants to be inserted before or after another line
        let wants_to_be_on_own_line = suggestion.replacement.ends_with('\n')
                                   || suggestion.replacement.starts_with('\n');
        if wants_to_be_on_own_line {
            println!("{}", "Insert line:".yellow().bold());
        } else {
            println!("{}", "At:".yellow().bold());
            println!(
                "{lead}{text}{tail}",
                lead = indent(4, &snippet.text.0),
                text = "v".red(),
                tail = snippet.text.2,
            );
            println!("{}\n", indent(snippet.text.0.len() as u32, "^").red());
            println!("{}\n", "insert:".yellow().bold());
        }
    } else {
        println!("{}\n", "Replace:".yellow().bold());
        println!(
            "{lead}{text}{tail}\n\n\
            {with}\n",
            with = "with:".yellow().bold(),
            lead = indent(4, &snippet.text.0),
            text = snippet.text.1.red(),
            tail = snippet.text.2,
        );
    }
}

fn handle_suggestions(
    suggestions: Vec<Suggestion>,
    mode: AutofixMode,
) -> Result<(), ProgramError> {
    let mut accepted_suggestions: Vec<Replacement> = vec![];

    if suggestions.is_empty() {
        println!("I don't have any suggestions for you right now. Check back later!");
        return Ok(());
    }

    'suggestions: for suggestion in suggestions {
        print!("\n\n{info}: {message}\n",
            info = "Info".green().bold(),
            message = split_at_lint_name(&suggestion.message));
        for snippet in suggestion.snippets {
            print!("{arrow} {file}:{range}\n",
                arrow = "  -->".blue().bold(),
                file = snippet.file_name,
                range = snippet.line_range);
        }

        let mut i = 0;
        for solution in &suggestion.solutions {
            println!("\n{}", solution.message);

            // check whether we can squash all suggestions into a list
            if solution.replacements.len() > 1 {
                let first = solution.replacements[0].clone();
                let all_suggestions_replace_the_same_span = solution
                    .replacements
                    .iter()
                    .all(|s| first.snippet.file_name == s.snippet.file_name
                         && first.snippet.line_range == s.snippet.line_range);
                if all_suggestions_replace_the_same_span {
                    prelude(&first);
                    for suggestion in &solution.replacements {
                        println!("[{}]: {}", i, suggestion.replacement.trim());
                        i += 1;
                    }
                    continue;
                }
            }
            for suggestion in &solution.replacements {
                print!("[{}]: ", i);
                prelude(suggestion);
                println!("{}", indent(4, &suggestion.replacement));
                i += 1;
            }
        }
        println!();

        if mode == AutofixMode::Yolo && suggestion.solutions.len() == 1 && suggestion.solutions[0].replacements.len() == 1 {
            let mut solutions = suggestion.solutions;
            let mut replacements = solutions.remove(0).replacements;
            accepted_suggestions.push(replacements.remove(0));
            println!("automatically applying suggestion (--yolo)");
            continue 'suggestions;
        }

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
                "q" => {
                    println!("Thanks for playing!");
                    break 'suggestions;
                }
                "a" => {
                    return Err(ProgramError::UserAbort);
                }
                "r" => {
                    if suggestion.solutions.len() == 1 && suggestion.solutions[0].replacements.len() == 1 {
                        let mut solutions = suggestion.solutions;
                        accepted_suggestions.push(solutions.remove(0).replacements.remove(0));
                        println!("Suggestion accepted. I'll remember that and apply it later.");
                        continue 'suggestions;
                    } else {
                        println!("{error}: multiple suggestions apply, please pick a number",
                            error = "Error".red().bold());
                    }
                }
                s => {
                    if let Ok(i) = usize::from_str(s) {
                        let replacement = suggestion.solutions
                            .iter()
                            .flat_map(|sol| sol.replacements.iter())
                            .nth(i);
                        if let Some(replacement) = replacement {
                            accepted_suggestions.push(replacement.clone());
                            println!("Suggestion accepted. I'll remember that and apply it later.");
                            continue 'suggestions;
                        } else {
                            println!("{error}: {i} is not a valid suggestion index",
                                error = "Error".red().bold(),
                                i = i);
                        }
                    } else {
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

    Ok(())
}

quick_error! {
    /// All possible errors in programm lifecycle
    #[derive(Debug)]
    pub enum ProgramError {
        UserAbort {
            display("Let's get outta here!")
        }
        /// Missing File
        NoFile {
            display("No input file given")
        }
        SubcommandError(subcommand: String, output: String) {
            display("Error executing subcommand `{}`", subcommand)
            description(output)
        }
        /// Error while dealing with file or stdin/stdout
        Io(err: std::io::Error) {
            from()
            cause(err)
            display("I/O error")
            description(err.description())
        }
        Utf8Error(err: std::string::FromUtf8Error) {
            from()
            display("Error reading input as UTF-8")
        }
        /// Error with deserialization
        Serde(err: serde_json::Error) {
            from()
            cause(err)
            display("Serde JSON error")
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
fn apply_suggestion(suggestion: &Replacement) -> Result<(), ProgramError> {
    use std::cmp::max;

    let file_content = try!(read_file_to_string(&suggestion.snippet.file_name));
    let mut new_content = String::new();

    // Add the lines before the section we want to replace
    new_content.push_str(&file_content.lines()
        .take(max(suggestion.snippet.line_range.start.line - 1, 0) as usize)
        .collect::<Vec<_>>()
        .join("\n"));
    new_content.push_str("\n");

    // Parts of line before replacement
    new_content.push_str(&file_content.lines()
        .nth(suggestion.snippet.line_range.start.line - 1)
        .unwrap_or("")
        .chars()
        .take(suggestion.snippet.line_range.start.column - 1)
        .collect::<String>());

    // Insert new content! Finally!
    new_content.push_str(&suggestion.replacement);

    // Parts of line after replacement
    new_content.push_str(&file_content.lines()
        .nth(suggestion.snippet.line_range.end.line - 1)
        .unwrap_or("")
        .chars()
        .skip(suggestion.snippet.line_range.end.column - 1)
        .collect::<String>());

    // Add the lines after the section we want to replace
    new_content.push_str("\n");
    new_content.push_str(&file_content.lines()
        .skip(suggestion.snippet.line_range.end.line as usize)
        .collect::<Vec<_>>()
        .join("\n"));
    new_content.push_str("\n");

    let mut file = try!(File::create(&suggestion.snippet.file_name));
    let new_content = new_content.as_bytes();

    try!(file.set_len(new_content.len() as u64));
    try!(file.write_all(new_content));

    Ok(())
}
