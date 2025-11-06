use crate::command_prelude::*;

use cargo::ops;
use cargo::ops::PackageMessageFormat;
use cargo::ops::PackageOpts;
use clap_complete::ArgValueCandidates;

pub fn cli() -> Command {
    subcommand("package")
        .about("Assemble the local package into a distributable tarball")
        .arg_index("Registry index URL to prepare the package for")
        .arg_registry("Registry to prepare the package for")
        .arg(
            flag(
                "list",
                "Print files included in a package without making one",
            )
            .short('l'),
        )
        .arg(flag(
            "no-verify",
            "Don't verify the contents by building them",
        ))
        .arg(flag(
            "no-metadata",
            "Ignore warnings about a lack of human-usable metadata",
        ))
        .arg(flag(
            "allow-dirty",
            "Allow dirty working directories to be packaged",
        ))
        .arg(flag(
            "exclude-lockfile",
            "Don't include the lock file when packaging",
        ))
        .arg(
            opt("message-format", "Output representation (unstable)")
                .value_name("FMT")
                // This currently requires and only works with `--list`.
                .requires("list")
                .value_parser(PackageMessageFormat::POSSIBLE_VALUES),
        )
        .arg_silent_suggestion()
        .arg_package_spec_no_all(
            "Package(s) to assemble",
            "Assemble all packages in the workspace",
            "Don't assemble specified packages",
            ArgValueCandidates::new(get_ws_member_candidates),
        )
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_parallel()
        .arg_manifest_path()
        .arg_lockfile_path()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help package</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let reg_or_index = args.registry_or_index(gctx)?;
    let ws = args.workspace(gctx)?;
    if ws.root_maybe().is_embedded() {
        return Err(anyhow::format_err!(
            "{} is unsupported by `cargo package`",
            ws.root_manifest().display()
        )
        .into());
    }
    let specs = args.packages_from_flags()?;

    let fmt = if let Some(fmt) = args._value_of("message-format") {
        gctx.cli_unstable()
            .fail_if_stable_opt("--message-format", 15353)?;
        fmt.parse()?
    } else {
        PackageMessageFormat::Human
    };

    ops::package(
        &ws,
        &PackageOpts {
            gctx,
            verify: !args.flag("no-verify"),
            list: args.flag("list"),
            fmt,
            check_metadata: !args.flag("no-metadata"),
            allow_dirty: args.flag("allow-dirty"),
            include_lockfile: !args.flag("exclude-lockfile"),
            to_package: specs,
            targets: args.targets()?,
            jobs: args.jobs()?,
            keep_going: args.keep_going(),
            cli_features: args.cli_features()?,
            reg_or_index,
            dry_run: false,
        },
    )?;

    Ok(())
}
