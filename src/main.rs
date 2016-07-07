#[macro_use]
extern crate quick_error;
extern crate serde_json;

extern crate rustfix;

use std::fs::File;
use std::io::{Read, Write};

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

fn try_main() -> Result<(), ProgramError> {
    let file_name = try!(std::env::args().skip(1).next().ok_or(ProgramError::NoFile));
    let mut file = try!(File::open(file_name));
    let mut buffer = String::new();
    try!(file.read_to_string(&mut buffer));

    for line in buffer.lines().filter(not_empty) {
        let deserialized: rustfix::diagnostics::Diagnostic = try!(serde_json::from_str(&line));
        println!("{:?}", rustfix::collect_suggestions(&deserialized, None));
    }

    Ok(())
}

fn main() {
    if let Err(error) = try_main() {
        writeln!(std::io::stderr(), "An error occured: {}", error).unwrap();
        std::process::exit(1);
    }
}


fn not_empty(s: &&str) -> bool {
    s.trim().len() > 0
}
