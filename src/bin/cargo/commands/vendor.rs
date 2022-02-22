use crate::command_prelude::*;
use cargo::ops;
use std::path::PathBuf;

pub fn cli() -> App {
    subcommand("vendor")
        .about("Vendor all dependencies for a project locally")
        .arg_quiet()
        .arg_manifest_path()
        .arg(
            Arg::new("path")
                .allow_invalid_utf8(true)
                .help("Where to vendor crates (`vendor` by default)"),
        )
        .arg(
            Arg::new("no-delete")
                .long("no-delete")
                .help("Don't delete older crates in the vendor directory"),
        )
        .arg(
            Arg::new("tomls")
                .short('s')
                .long("sync")
                .help("Additional `Cargo.toml` to sync and vendor")
                .value_name("TOML")
                .allow_invalid_utf8(true)
                .multiple_occurrences(true)
                .multiple_values(true),
        )
        .arg(
            Arg::new("respect-source-config")
                .long("respect-source-config")
                .help("Respect `[source]` config in `.cargo/config`")
                .multiple_occurrences(true),
        )
        .arg(
            Arg::new("versioned-dirs")
                .long("versioned-dirs")
                .help("Always include version in subdir name"),
        )
        // Not supported.
        .arg(
            Arg::new("no-merge-sources")
                .long("no-merge-sources")
                .hide(true),
        )
        // Not supported.
        .arg(Arg::new("relative-path").long("relative-path").hide(true))
        // Not supported.
        .arg(Arg::new("only-git-deps").long("only-git-deps").hide(true))
        // Not supported.
        .arg(
            Arg::new("disallow-duplicates")
                .long("disallow-duplicates")
                .hide(true),
        )
        .after_help("Run `cargo help vendor` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
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
