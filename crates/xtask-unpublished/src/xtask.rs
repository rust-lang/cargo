use std::collections::HashSet;
use cargo::core::registry::PackageRegistry;
use cargo::core::QueryKind;
use cargo::core::Registry;
use cargo::core::SourceId;
use cargo::ops::Packages;
use cargo::util::command_prelude::*;

pub fn cli() -> clap::Command {
    clap::Command::new("xtask-unpublished")
        .arg_package_spec_simple("Package to inspect the published status")
        .arg(
            opt(
                "verbose",
                "Use verbose output (-vv very verbose/build.rs output)",
            )
            .short('v')
            .action(ArgAction::Count)
            .global(true),
        )
        .arg_quiet()
        .arg(
            opt("color", "Coloring: auto, always, never")
                .value_name("WHEN")
                .global(true),
        )
        .arg(flag("frozen", "Require Cargo.lock and cache are up to date").global(true))
        .arg(flag("locked", "Require Cargo.lock is up to date").global(true))
        .arg(flag("offline", "Run without accessing the network").global(true))
        .arg(multi_opt("config", "KEY=VALUE", "Override a configuration value").global(true))
        .arg(
            Arg::new("unstable-features")
                .help("Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details")
                .short('Z')
                .value_name("FLAG")
                .action(ArgAction::Append)
                .global(true),
        )
}

pub fn exec(args: &clap::ArgMatches, config: &mut cargo::util::Config) -> cargo::CliResult {
    config_configure(config, args)?;

    unpublished(args, config)?;

    Ok(())
}

fn config_configure(config: &mut Config, args: &ArgMatches) -> CliResult {
    let verbose = args.verbose();
    // quiet is unusual because it is redefined in some subcommands in order
    // to provide custom help text.
    let quiet = args.flag("quiet");
    let color = args.get_one::<String>("color").map(String::as_str);
    let frozen = args.flag("frozen");
    let locked = args.flag("locked");
    let offline = args.flag("offline");
    let mut unstable_flags = vec![];
    if let Some(values) = args.get_many::<String>("unstable-features") {
        unstable_flags.extend(values.cloned());
    }
    let mut config_args = vec![];
    if let Some(values) = args.get_many::<String>("config") {
        config_args.extend(values.cloned());
    }
    config.configure(
        verbose,
        quiet,
        color,
        frozen,
        locked,
        offline,
        &None,
        &unstable_flags,
        &config_args,
    )?;
    Ok(())
}

fn unpublished(args: &clap::ArgMatches, config: &mut cargo::util::Config) -> cargo::CliResult {
    let ws = args.workspace(config)?;

    let members_to_inspect: HashSet<_> = {
        let pkgs = args.packages_from_flags()?;
        if let Packages::Packages(_) = pkgs {
            HashSet::from_iter(pkgs.get_packages(&ws)?)
        } else {
            HashSet::from_iter(ws.members())
        }
    };

    let mut results = Vec::new();
    {
        let mut registry = PackageRegistry::new(config)?;
        let _lock = config.acquire_package_cache_lock()?;
        registry.lock_patches();
        let source_id = SourceId::crates_io(config)?;

        for member in members_to_inspect {
            let name = member.name();
            let current = member.version();
            if member.publish() == &Some(vec![]) {
                log::trace!("skipping {name}, `publish = false`");
                continue;
            }

            let version_req = format!("<={current}");
            let query =
                cargo::core::dependency::Dependency::parse(name, Some(&version_req), source_id)?;
            let possibilities = loop {
                // Exact to avoid returning all for path/git
                match registry.query_vec(&query, QueryKind::Exact) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            };
            if let Some(last) = possibilities.iter().map(|s| s.version()).max() {
                if last != current {
                    results.push((
                        name.to_string(),
                        Some(last.to_string()),
                        current.to_string(),
                    ));
                } else {
                    log::trace!("{name} {current} is published");
                }
            } else {
                results.push((name.to_string(), None, current.to_string()));
            }
        }
    }

    if !results.is_empty() {
        results.insert(
            0,
            (
                "name".to_owned(),
                Some("published".to_owned()),
                "current".to_owned(),
            ),
        );
        results.insert(
            1,
            (
                "====".to_owned(),
                Some("=========".to_owned()),
                "=======".to_owned(),
            ),
        );
    }
    for (name, last, current) in results {
        if let Some(last) = last {
            println!("{name} {last} {current}");
        } else {
            println!("{name} - {current}");
        }
    }

    Ok(())
}

#[test]
fn verify_cli() {
    cli().debug_assert();
}
