use crate::command_prelude::*;

use cargo::ops::{self, CompileFilter, FilterRule, LibRule};

pub fn cli() -> App {
    subcommand("fix")
        .about("Automatically fix lint warnings reported by rustc")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
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
            Arg::with_name("broken-code")
                .long("broken-code")
                .help("Fix code even if it already has compiler errors"),
        )
        .arg(
            Arg::with_name("edition")
                .long("edition")
                .help("Fix in preparation for the next edition"),
        )
        .arg(
            // This is a deprecated argument, we'll want to phase it out
            // eventually.
            Arg::with_name("prepare-for")
                .long("prepare-for")
                .help("Fix warnings in preparation of an edition upgrade")
                .takes_value(true)
                .possible_values(&["2018"])
                .conflicts_with("edition")
                .hidden(true),
        )
        .arg(
            Arg::with_name("idioms")
                .long("edition-idioms")
                .help("Fix warnings to migrate to the idioms of an edition"),
        )
        .arg(
            Arg::with_name("allow-no-vcs")
                .long("allow-no-vcs")
                .help("Fix code even if a VCS was not detected"),
        )
        .arg(
            Arg::with_name("allow-dirty")
                .long("allow-dirty")
                .help("Fix code even if the working directory is dirty"),
        )
        .arg(
            Arg::with_name("allow-staged")
                .long("allow-staged")
                .help("Fix code even if the working directory has staged changes"),
        )
        .after_help(
            "\
This Cargo subcommand will automatically take rustc's suggestions from
diagnostics like warnings and apply them to your source code. This is intended
to help automate tasks that rustc itself already knows how to tell you to fix!
The `cargo fix` subcommand is also being developed for the Rust 2018 edition
to provide code the ability to easily opt-in to the new edition without having
to worry about any breakage.

Executing `cargo fix` will under the hood execute `cargo check`. Any warnings
applicable to your crate will be automatically fixed (if possible) and all
remaining warnings will be displayed when the check process is finished. For
example if you'd like to prepare for the 2018 edition, you can do so by
executing:

    cargo fix --edition

which behaves the same as `cargo check --all-targets`. Similarly if you'd like
to fix code for different platforms you can do:

    cargo fix --edition --target x86_64-pc-windows-gnu

or if your crate has optional features:

    cargo fix --edition --no-default-features --features foo

If you encounter any problems with `cargo fix` or otherwise have any questions
or feature requests please don't hesitate to file an issue at
https://github.com/rust-lang/cargo
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let test = match args.value_of("profile") {
        Some("test") => true,
        None => false,
        Some(profile) => {
            let err = anyhow::format_err!(
                "unknown profile: `{}`, only `test` is \
                 currently supported",
                profile
            );
            return Err(CliError::new(err, 101));
        }
    };
    let mode = CompileMode::Check { test };

    // Unlike other commands default `cargo fix` to all targets to fix as much
    // code as we can.
    let mut opts = args.compile_options(config, mode, Some(&ws), ProfileChecking::Unchecked)?;

    if let CompileFilter::Default { .. } = opts.filter {
        opts.filter = CompileFilter::Only {
            all_targets: true,
            lib: LibRule::Default,
            bins: FilterRule::All,
            examples: FilterRule::All,
            benches: FilterRule::All,
            tests: FilterRule::All,
        }
    }

    ops::fix(
        &ws,
        &mut ops::FixOptions {
            edition: args.is_present("edition"),
            prepare_for: args.value_of("prepare-for"),
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
