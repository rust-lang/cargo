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
pub struct Flags {
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_frozen: bool,
    flag_locked: bool,
}

pub const USAGE: &'static str = "
Check correctness of crate manifest

Usage:
    cargo verify-project [options]
    cargo verify-project -h | --help

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to verify
    -v, --verbose ...       Use verbose output
    -q, --quiet             No output printed to stdout
    --color WHEN            Coloring: auto, always, never
    --frozen                Require Cargo.lock and cache are up to date
    --locked                Require Cargo.lock is up to date
";

pub fn execute(args: Flags, config: &Config) -> CliResult<Option<Error>> {
    config.configure(args.flag_verbose,
                          args.flag_quiet,
                          &args.flag_color,
                          args.flag_frozen,
                          args.flag_locked)?;

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
