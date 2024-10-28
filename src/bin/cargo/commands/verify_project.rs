//! Deprecated.

use crate::command_prelude::*;

use std::collections::HashMap;
use std::process;

pub fn cli() -> Command {
    subcommand("verify-project")
        .hide(true)
        .about(
            "\
DEPRECATED: Check correctness of crate manifest.

See https://github.com/rust-lang/cargo/issues/14679.",
        )
        .arg_silent_suggestion()
        .arg_manifest_path()
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    if let Err(e) = args.workspace(gctx) {
        gctx.shell()
            .print_json(&HashMap::from([("invalid", e.to_string())]))?;
        process::exit(1)
    }

    gctx.shell()
        .print_json(&HashMap::from([("success", "true")]))?;
    Ok(())
}
