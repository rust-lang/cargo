//! Deprecated.

use crate::command_prelude::*;

use crate::util::data_structures::HashMap;
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
            .print_json(&HashMap::from_iter([("invalid", e.to_string())]))?;
        process::exit(1)
    }

    gctx.shell()
        .print_json(&HashMap::from_iter([("success", "true")]))?;
    Ok(())
}
