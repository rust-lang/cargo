use crate::command_prelude::*;

use std::collections::HashMap;
use std::process;

pub fn cli() -> Command {
    subcommand("verify-project")
        .about("Check correctness of crate manifest")
        .arg_silent_suggestion()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help verify-project</>` for more detailed information.\n"
        ))
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
