use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("fix")
        .about("Automatically fix lint warnings reported by rustc")
        .arg(flag("edition", "Fix in preparation for the next edition"))
        .arg(flag(
            "edition-idioms",
            "Fix warnings to migrate to the idioms of an edition",
        ))
        .arg(flag(
            "broken-code",
            "Fix code even if it already has compiler errors",
        ))
        .arg(flag(
            "allow-no-vcs",
            "Fix code even if a VCS was not detected",
        ))
        .arg(flag(
            "allow-dirty",
            "Fix code even if the working directory is dirty",
        ))
        .arg(flag(
            "allow-staged",
            "Fix code even if the working directory has staged changes",
        ))
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_quiet()
        .arg_package_spec(
            "Package(s) to fix",
            "Fix all packages in the workspace",
            "Exclude packages from the fixes",
        )
        .arg_targets_all(
            "Fix only this package's library",
            "Fix only the specified binary",
            "Fix all binaries",
            "Fix only the specified example",
            "Fix all examples",
            "Fix only the specified test target",
            "Fix all tests",
            "Fix only the specified bench target",
            "Fix all benches",
            "Fix all targets (default)",
        )
        .arg_features()
        .arg_parallel()
        .arg_release("Fix artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_target_triple("Fix for the target triple")
        .arg_target_dir()
        .arg_timings()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help fix</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    // This is a legacy behavior that causes `cargo fix` to pass `--test`.
    let test = matches!(
        args.get_one::<String>("profile").map(String::as_str),
        Some("test")
    );
    let mode = CompileMode::Check { test };

    // Unlike other commands default `cargo fix` to all targets to fix as much
    // code as we can.
    let mut opts =
        args.compile_options(config, mode, Some(&ws), ProfileChecking::LegacyTestOnly)?;

    if !opts.filter.is_specific() {
        // cargo fix with no target selection implies `--all-targets`.
        opts.filter = ops::CompileFilter::new_all_targets();
    }

    ops::fix(
        &ws,
        &mut ops::FixOptions {
            edition: args.flag("edition"),
            idioms: args.flag("edition-idioms"),
            compile_opts: opts,
            allow_dirty: args.flag("allow-dirty"),
            allow_no_vcs: args.flag("allow-no-vcs"),
            allow_staged: args.flag("allow-staged"),
            broken_code: args.flag("broken-code"),
        },
    )?;
    Ok(())
}
