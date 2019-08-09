use crate::command_prelude::*;

use cargo::ops;
use cargo::util;

pub fn cli() -> App {
    subcommand("clippy-preview")
        .about("Checks a package to catch common mistakes and improve your Rust code.")
        .arg(Arg::with_name("args").multiple(true))
        .arg_package_spec(
            "Package(s) to check",
            "Check all packages in the workspace",
            "Exclude packages from the check",
        )
        .arg_jobs()
        .arg_targets_all(
            "Check only this package's library",
            "Check only the specified binary",
            "Check all binaries",
            "Check only the specified example",
            "Check all examples",
            "Check only the specified test target",
            "Check all tests",
            "Check only the specified bench target",
            "Check all benches",
            "Check all targets",
        )
        .arg_release("Check artifacts in release mode, with optimizations")
        .arg_features()
        .arg_target_triple("Check for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .after_help(
            "\
If the `--package` argument is given, then SPEC is a package ID specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are checked if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

To allow or deny a lint from the command line you can use `cargo clippy --`
with:

    -W --warn OPT       Set lint warnings
    -A --allow OPT      Set lint allowed
    -D --deny OPT       Set lint denied
    -F --forbid OPT     Set lint forbidden

You can use tool lints to allow or deny lints from your code, eg.:

    #[allow(clippy::needless_lifetimes)]
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;

    let mode = CompileMode::Check { test: false };
    let mut compile_opts = args.compile_options(config, mode, Some(&ws))?;

    if !config.cli_unstable().unstable_options {
        return Err(failure::format_err!(
            "`clippy-preview` is unstable, pass `-Z unstable-options` to enable it"
        )
        .into());
    }

    let mut wrapper = util::process(util::config::clippy_driver());

    if let Some(clippy_args) = args.values_of("args") {
        wrapper.args(&clippy_args.collect::<Vec<_>>());
    }

    compile_opts.build_config.primary_unit_rustc = Some(wrapper);
    compile_opts.build_config.force_rebuild = true;

    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
