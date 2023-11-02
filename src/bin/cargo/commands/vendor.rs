use crate::command_prelude::*;
use cargo::ops;
use std::path::PathBuf;

pub fn cli() -> Command {
    subcommand("vendor")
        .about("Vendor all dependencies for a project locally")
        .arg(
            Arg::new("path")
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(PathBuf))
                .help("Where to vendor crates (`vendor` by default)"),
        )
        .arg(flag(
            "no-delete",
            "Don't delete older crates in the vendor directory",
        ))
        .arg(
            Arg::new("tomls")
                .short('s')
                .long("sync")
                .help("Additional `Cargo.toml` to sync and vendor")
                .value_name("TOML")
                .value_parser(clap::value_parser!(PathBuf))
                .action(clap::ArgAction::Append),
        )
        .arg(flag(
            "respect-source-config",
            "Respect `[source]` config in `.cargo/config`",
        ))
        .arg(flag(
            "versioned-dirs",
            "Always include version in subdir name",
        ))
        .arg(unsupported("no-merge-sources"))
        .arg(unsupported("relative-path"))
        .arg(unsupported("only-git-deps"))
        .arg(unsupported("disallow-duplicates"))
        .arg_quiet_without_unknown_silent_arg_tip()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help vendor</>` for more detailed information.\n"
        ))
}

fn unsupported(name: &'static str) -> Arg {
    // When we moved `cargo vendor` into Cargo itself we didn't stabilize a few
    // flags, so try to provide a helpful error message in that case to ensure
    // that users currently using the flag aren't tripped up.
    let value_parser = clap::builder::UnknownArgumentValueParser::suggest("the crates.io `cargo vendor` command has been merged into Cargo")
        .and_suggest(format!("and the flag `--{name}` isn't supported currently"))
        .and_suggest("to continue using the flag, execute `cargo-vendor vendor ...`")
        .and_suggest("to suggest this flag supported in Cargo, file an issue at <https://github.com/rust-lang/cargo/issues/new>");

    flag(name, "").value_parser(value_parser).hide(true)
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    // We're doing the vendoring operation ourselves, so we don't actually want
    // to respect any of the `source` configuration in Cargo itself. That's
    // intended for other consumers of Cargo, but we want to go straight to the
    // source, e.g. crates.io, to fetch crates.
    if !args.flag("respect-source-config") {
        config.values_mut()?.remove("source");
    }

    let ws = args.workspace(config)?;
    let path = args
        .get_one::<PathBuf>("path")
        .cloned()
        .unwrap_or_else(|| PathBuf::from("vendor"));
    ops::vendor(
        &ws,
        &ops::VendorOptions {
            no_delete: args.flag("no-delete"),
            destination: &path,
            versioned_dirs: args.flag("versioned-dirs"),
            extra: args
                .get_many::<PathBuf>("tomls")
                .unwrap_or_default()
                .cloned()
                .collect(),
        },
    )?;
    Ok(())
}
