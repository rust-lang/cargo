use crate::command_prelude::*;

use std::collections::HashMap;
use std::process;

pub fn cli() -> App {
    subcommand("verify-project")
        .about("Check correctness of crate manifest")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
        .after_help("Run `cargo help verify-project` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    if let Err(e) = args.workspace(config) {
        let mut h = HashMap::new();
        h.insert("invalid".to_string(), e.to_string());
        config.shell().print_json(&h);
        process::exit(1)
    }

    let mut h = HashMap::new();
    h.insert("success".to_string(), "true".to_string());
    config.shell().print_json(&h);
    Ok(())
}
