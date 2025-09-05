use crate::command_prelude::*;
use cargo::core::Workspace;

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
            "Fix code even if the working directory is dirty or has staged changes",
        ))
        .arg(flag(
            "allow-staged",
            "Fix code even if the working directory has staged changes",
        ))
        .arg_message_format()
        .arg_silent_suggestion()
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
            "Fix all targets that have `test = true` set",
            "Fix only the specified bench target",
            "Fix all targets that have `bench = true` set",
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
        .arg_lockfile_path()
        .arg_ignore_rust_version()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help fix</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    // This is a legacy behavior that causes `cargo fix` to pass `--test`.
    let test = matches!(
        args.get_one::<String>("profile").map(String::as_str),
        Some("test")
    );
    let intent = UserIntent::Check { test };

    // Unlike other commands default `cargo fix` to all targets to fix as much
    // code as we can.
    let root_manifest = args.root_manifest(gctx)?;

    // Can't use workspace() to avoid using -Zavoid-dev-deps (if passed)
    let mut ws = Workspace::new(&root_manifest, gctx)?;
    ws.set_resolve_honors_rust_version(args.honor_rust_version());
    let lockfile_path = args.lockfile_path(gctx)?;
    ws.set_requested_lockfile_path(lockfile_path.clone());

    let mut opts =
        args.compile_options(gctx, intent, Some(&ws), ProfileChecking::LegacyTestOnly)?;

    let edition = args.flag("edition") || args.flag("edition-idioms");
    if !opts.filter.is_specific() && edition {
        // When `cargo fix` is run without specifying targets but with `--edition` or `--edition-idioms`,
        // it should default to fixing all targets.
        // See: https://github.com/rust-lang/cargo/issues/13527
        opts.filter = ops::CompileFilter::new_all_targets();
    }

    let allow_dirty = args.flag("allow-dirty");

    let mut opts = ops::FixOptions {
        edition: args
            .flag("edition")
            .then_some(ops::EditionFixMode::NextRelative),
        idioms: args.flag("edition-idioms"),
        compile_opts: opts,
        allow_dirty,
        allow_staged: allow_dirty || args.flag("allow-staged"),
        allow_no_vcs: args.flag("allow-no-vcs"),
        broken_code: args.flag("broken-code"),
        requested_lockfile_path: lockfile_path,
    };

    if let Some(fe) = &gctx.cli_unstable().fix_edition {
        ops::fix_edition(gctx, &ws, &mut opts, fe)?;
    } else {
        ops::fix(gctx, &ws, &mut opts)?;
    }

    Ok(())
}
