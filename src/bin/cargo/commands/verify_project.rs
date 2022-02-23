use crate::command_prelude::*;

use std::collections::HashMap;
use std::process;

pub fn cli() -> App {
    subcommand("verify-project")
        .about("Check correctness of crate manifest")
        .arg_quiet()
        .arg_manifest_path()
        .after_help("Run `cargo help verify-project` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    if let Err(e) = args.workspace(config) {
        let h: HashMap<_, _> = [("invalid", e.to_string())].into();
        config.shell().print_json(&h)?;
        process::exit(1)
    }

    let h: HashMap<_, _> = [("success", "true")].into();
    config.shell().print_json(&h)?;
    Ok(())
}
