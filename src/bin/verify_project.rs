use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::process;

use cargo::util::important_paths::{find_root_manifest_for_wd};
use cargo::util::{CliResult, Config};
use rustc_serialize::json;
use toml;

pub type Error = HashMap<String, String>;

#[derive(RustcDecodable)]
struct Flags {
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_quiet: bool,
    flag_color: Option<String>,
}

pub const USAGE: &'static str = "
Usage:
    cargo verify-project [options]
    cargo verify-project -h | --help

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to verify
    -v, --verbose           Use verbose output
    -q, --quiet             No output printed to stdout
    --color WHEN            Coloring: auto, always, never
";

pub fn execute(args: Flags, config: &Config) -> CliResult<Option<Error>> {
    try!(config.shell().set_verbosity(args.flag_verbose, args.flag_quiet));
    try!(config.shell().set_color_config(args.flag_color.as_ref().map(|s| &s[..])));

    let mut contents = String::new();
    let filename = args.flag_manifest_path.unwrap_or("Cargo.toml".into());
    let filename = match find_root_manifest_for_wd(Some(filename), config.cwd()) {
        Ok(manifest_path) => manifest_path,
        Err(e) => fail("invalid", &e.to_string()),
    };

    let file = File::open(&filename);
    match file.and_then(|mut f| f.read_to_string(&mut contents)) {
        Ok(_) => {},
        Err(e) => fail("invalid", &format!("error reading file: {}", e))
    };
    match toml::Parser::new(&contents).parse() {
        None => fail("invalid", "invalid-format"),
        Some(..) => {}
    };

    let mut h = HashMap::new();
    h.insert("success".to_string(), "true".to_string());
    Ok(Some(h))
}

fn fail(reason: &str, value: &str) -> ! {
    let mut h = HashMap::new();
    h.insert(reason.to_string(), value.to_string());
    println!("{}", json::encode(&h).unwrap());
    process::exit(1)
}
