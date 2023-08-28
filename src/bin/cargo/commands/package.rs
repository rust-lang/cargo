use crate::command_prelude::*;

use cargo::ops::{self, PackageOpts};

pub fn cli() -> Command {
    subcommand("package")
        .about("Assemble the local package into a distributable tarball")
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
        .arg_quiet()
        .arg_package_spec_no_all(
            "Package(s) to assemble",
            "Assemble all packages in the workspace",
            "Don't assemble specified packages",
        )
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_parallel()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help package</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    if ws.root_maybe().is_embedded() {
        return Err(anyhow::format_err!(
            "{} is unsupported by `cargo package`",
            ws.root_manifest().display()
        )
        .into());
    }
    let specs = args.packages_from_flags()?;

    ops::package(
        &ws,
        &PackageOpts {
            config,
            verify: !args.flag("no-verify"),
            list: args.flag("list"),
            check_metadata: !args.flag("no-metadata"),
            allow_dirty: args.flag("allow-dirty"),
            to_package: specs,
            targets: args.targets()?,
            jobs: args.jobs()?,
            keep_going: args.keep_going(),
            cli_features: args.cli_features()?,
        },
    )?;

    Ok(())
}
