extern crate toml;

use std::collections::HashMap;
use std::io::File;
use std::os;
use docopt;

use cargo::core::MultiShell;
use cargo::util::CliResult;

pub type Error = HashMap<String, String>;

docopt!(Flags, "
Usage:
    cargo verify-project [options] --manifest-path PATH

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to verify
    -v, --verbose           Use verbose output
")

pub fn execute(args: Flags,
               shell: &mut MultiShell) -> CliResult<Option<Error>> {
    shell.set_verbose(args.flag_verbose);

    let file = Path::new(args.flag_manifest_path);
    let contents = match File::open(&file).read_to_string() {
        Ok(s) => s,
        Err(e) => return fail("invalid", format!("error reading file: {}",
                                                 e).as_slice())
    };
    match toml::Parser::new(contents.as_slice()).parse() {
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
    os::set_exit_status(1);
    Ok(Some(h))
}
