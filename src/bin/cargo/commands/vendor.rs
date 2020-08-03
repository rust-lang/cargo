use crate::command_prelude::*;
use cargo::ops;
use std::path::PathBuf;

pub fn cli() -> App {
    subcommand("vendor")
        .about("Vendor all dependencies for a project locally")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
        .arg(Arg::with_name("path").help("Where to vendor crates (`vendor` by default)"))
        .arg(
            Arg::with_name("no-delete")
                .long("no-delete")
                .help("Don't delete older crates in the vendor directory"),
        )
        .arg(
            Arg::with_name("tomls")
                .short("s")
                .long("sync")
                .help("Additional `Cargo.toml` to sync and vendor")
                .value_name("TOML")
                .multiple(true),
        )
        .arg(
            Arg::with_name("respect-source-config")
                .long("respect-source-config")
                .help("Respect `[source]` config in `.cargo/config`")
                .multiple(true),
        )
        .arg(
            Arg::with_name("versioned-dirs")
                .long("versioned-dirs")
                .help("Always include version in subdir name"),
        )
        .arg(
            Arg::with_name("no-merge-sources")
                .long("no-merge-sources")
                .hidden(true),
        )
        .arg(
            Arg::with_name("relative-path")
                .long("relative-path")
                .hidden(true),
        )
        .arg(
            Arg::with_name("only-git-deps")
                .long("only-git-deps")
                .hidden(true),
        )
        .arg(
            Arg::with_name("disallow-duplicates")
                .long("disallow-duplicates")
                .hidden(true),
        )
        .after_help("Run `cargo help vendor` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    // We're doing the vendoring operation ourselves, so we don't actually want
    // to respect any of the `source` configuration in Cargo itself. That's
    // intended for other consumers of Cargo, but we want to go straight to the
    // source, e.g. crates.io, to fetch crates.
    if !args.is_present("respect-source-config") {
        config.values_mut()?.remove("source");
    }

    // When we moved `cargo vendor` into Cargo itself we didn't stabilize a few
    // flags, so try to provide a helpful error message in that case to ensure
    // that users currently using the flag aren't tripped up.
    let crates_io_cargo_vendor_flag = if args.is_present("no-merge-sources") {
        Some("--no-merge-sources")
    } else if args.is_present("relative-path") {
        Some("--relative-path")
    } else if args.is_present("only-git-deps") {
        Some("--only-git-deps")
    } else if args.is_present("disallow-duplicates") {
        Some("--disallow-duplicates")
    } else {
        None
    };
    if let Some(flag) = crates_io_cargo_vendor_flag {
        return Err(anyhow::format_err!(
            "\
the crates.io `cargo vendor` command has now been merged into Cargo itself
and does not support the flag `{}` currently; to continue using the flag you
can execute `cargo-vendor vendor ...`, and if you would like to see this flag
supported in Cargo itself please feel free to file an issue at
https://github.com/rust-lang/cargo/issues/new
",
            flag
        )
        .into());
    }

    let ws = args.workspace(config)?;
    let path = args
        .value_of_os("path")
        .map(|val| PathBuf::from(val.to_os_string()))
        .unwrap_or_else(|| PathBuf::from("vendor"));
    ops::vendor(
        &ws,
        &ops::VendorOptions {
            no_delete: args.is_present("no-delete"),
            destination: &path,
            versioned_dirs: args.is_present("versioned-dirs"),
            extra: args
                .values_of_os("tomls")
                .unwrap_or_default()
                .map(|s| PathBuf::from(s.to_os_string()))
                .collect(),
        },
    )?;
    Ok(())
}
