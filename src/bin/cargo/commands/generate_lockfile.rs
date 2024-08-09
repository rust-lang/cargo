use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("generate-lockfile")
        .about("Generate the lockfile for a package")
        .arg_silent_suggestion()
        .arg_manifest_path()
        .arg_lockfile_path()
        .arg_ignore_rust_version_with_help(
            "Ignore `rust-version` specification in packages (unstable)",
        )
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help generate-lockfile</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    if args.honor_rust_version().is_some() {
        gctx.cli_unstable().fail_if_stable_opt_custom_z(
            "--ignore-rust-version",
            9930,
            "msrv-policy",
            gctx.cli_unstable().msrv_policy,
        )?;
    }
    let ws = args.workspace(gctx)?;
    ops::generate_lockfile(&ws)?;
    Ok(())
}
