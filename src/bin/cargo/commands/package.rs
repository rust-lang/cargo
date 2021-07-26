use crate::command_prelude::*;

use cargo::ops::{self, PackageOpts};

pub fn cli() -> App {
    subcommand("package")
        .about("Assemble the local package into a distributable tarball")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(
            opt(
                "list",
                "Print files included in a package without making one",
            )
            .short("l"),
        )
        .arg(opt(
            "no-verify",
            "Don't verify the contents by building them",
        ))
        .arg(opt(
            "no-metadata",
            "Ignore warnings about a lack of human-usable metadata",
        ))
        .arg(opt(
            "allow-dirty",
            "Allow dirty working directories to be packaged",
        ))
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_features()
        .arg_package_spec(
            "Package(s) to assemble",
            "Assemble all packages in the workspace",
            "Don't assemble specified packages",
        )
        .arg_manifest_path()
        .arg_jobs()
        .after_help("Run `cargo help package` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let specs = args.packages_from_flags()?;

    ops::package(
        &ws,
        &PackageOpts {
            config,
            verify: !args.is_present("no-verify"),
            list: args.is_present("list"),
            check_metadata: !args.is_present("no-metadata"),
            allow_dirty: args.is_present("allow-dirty"),
            to_package: specs,
            targets: args.targets(),
            jobs: args.jobs()?,
            cli_features: args.cli_features()?,
        },
    )?;

    Ok(())
}
