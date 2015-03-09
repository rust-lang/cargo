use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::prelude::*;

use toml;
use cargo::util::{CliResult, Config};

pub type Error = HashMap<String, String>;

#[derive(RustcDecodable)]
struct Flags {
    flag_manifest_path: String,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Usage:
    cargo verify-project [options] --manifest-path PATH
    cargo verify-project -h | --help

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to verify
    -v, --verbose           Use verbose output
";

pub fn execute(args: Flags, config: &Config) -> CliResult<Option<Error>> {
    config.shell().set_verbose(args.flag_verbose);

    let mut contents = String::new();
    let file = File::open(&args.flag_manifest_path);
    match file.and_then(|mut f| f.read_to_string(&mut contents)) {
        Ok(()) => {},
        Err(e) => return fail("invalid", &format!("error reading file: {}", e))
    };
    match toml::Parser::new(&contents).parse() {
        None => return fail("invalid", "invalid-format"),
        Some(..) => {}
    };

    let mut h = HashMap::new();
    h.insert("success".to_string(), "true".to_string());
    Ok(Some(h))
}

fn fail(reason: &str, value: &str) -> CliResult<Option<Error>>{
    let mut h = HashMap::new();
    h.insert(reason.to_string(), value.to_string());
    env::set_exit_status(1);
    Ok(Some(h))
}
