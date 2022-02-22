use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("fix")
        .about("Automatically fix lint warnings reported by rustc")
        .arg_quiet()
        .arg_package_spec(
            "Package(s) to fix",
            "Fix all packages in the workspace",
            "Exclude packages from the fixes",
        )
        .arg_jobs()
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
        .arg_release("Fix artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Fix for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg(
            Arg::new("broken-code")
                .long("broken-code")
                .help("Fix code even if it already has compiler errors"),
        )
        .arg(
            Arg::new("edition")
                .long("edition")
                .help("Fix in preparation for the next edition"),
        )
        .arg(
            Arg::new("idioms")
                .long("edition-idioms")
                .help("Fix warnings to migrate to the idioms of an edition"),
        )
        .arg(
            Arg::new("allow-no-vcs")
                .long("allow-no-vcs")
                .help("Fix code even if a VCS was not detected"),
        )
        .arg(
            Arg::new("allow-dirty")
                .long("allow-dirty")
                .help("Fix code even if the working directory is dirty"),
        )
        .arg(
            Arg::new("allow-staged")
                .long("allow-staged")
                .help("Fix code even if the working directory has staged changes"),
        )
        .arg_ignore_rust_version()
        .arg_timings()
        .after_help("Run `cargo help fix` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    // This is a legacy behavior that causes `cargo fix` to pass `--test`.
    let test = matches!(args.value_of("profile"), Some("test"));
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
            edition: args.is_present("edition"),
            idioms: args.is_present("idioms"),
            compile_opts: opts,
            allow_dirty: args.is_present("allow-dirty"),
            allow_no_vcs: args.is_present("allow-no-vcs"),
            allow_staged: args.is_present("allow-staged"),
            broken_code: args.is_present("broken-code"),
        },
    )?;
    Ok(())
}
