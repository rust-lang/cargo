use crate::command_prelude::*;

use std::collections::HashMap;
use std::process;

use cargo::print_json;

pub fn cli() -> App {
    subcommand("verify-project")
        .about("Check correctness of crate manifest")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    fn fail(reason: &str, value: &str) -> ! {
        let mut h = HashMap::new();
        h.insert(reason.to_string(), value.to_string());
        print_json(&h);
        process::exit(1)
    }

    if let Err(e) = args.workspace(config) {
        fail("invalid", &e.to_string())
    }

    let mut h = HashMap::new();
    h.insert("success".to_string(), "true".to_string());
    print_json(&h);
    Ok(())
}
