use crate::command_prelude::*;
use cargo::core::gc::{parse_human_size, parse_time_span};
use cargo::core::gc::{AutoGcKind, GcOpts};
use cargo::ops::{self, CleanOptions};
use cargo::util::print_available_packages;
use cargo::CargoResult;
use clap::builder::{PossibleValuesParser, TypedValueParser};
use std::time::Duration;

pub fn cli() -> Command {
    subcommand("clean")
        .about("Remove artifacts that cargo has generated in the past")
        .arg_doc("Whether or not to clean just the documentation directory")
        .arg_quiet()
        .arg_package_spec_simple("Package to clean artifacts for")
        .arg_release("Whether or not to clean release artifacts")
        .arg_profile("Clean artifacts of the specified profile")
        .arg_target_triple("Target triple to clean output for")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_dry_run("Display what would be deleted without deleting anything")

        // NOTE: Not all of these options may get stabilized. Some of them are
        // very low-level details, and may not be something typical users need.
        .arg(
            optional_opt(
                "gc",
                "Delete old and unused files (unstable) (comma separated): all, download, target, shared-target",
            )
            .hide(true)
            .value_name("KINDS")
            .value_parser(
                PossibleValuesParser::new(["all", "download", "target", "shared-target"]).map(|x|
                    match x.as_str() {
                        "all" => AutoGcKind::All,
                        "download" => AutoGcKind::Download,
                        "target" => panic!("target is not yet implemented"),
                        "shared-target" => panic!("shared-target is not yet implemented"),
                        x => panic!("possible value out of sync with `{x}`"),
                    }
            ))
            .require_equals(true),
        )
        .arg(
            opt(
                "max-src-age",
                "Deletes source cache files that have not been used since the given age (unstable)",
            )
            .hide(true)
            .value_name("DURATION"),
        )
        .arg(
            opt(
                "max-crate-age",
                "Deletes crate cache files that have not been used since the given age (unstable)",
            )
            .hide(true)
            .value_name("DURATION"),
        )
        .arg(
            opt(
                "max-index-age",
                "Deletes registry indexes that have not been used since then given age (unstable)",
            )
            .hide(true)
            .value_name("DURATION"),
        )
        .arg(
            opt(
                "max-git-co-age",
                "Deletes git dependency checkouts that have not been used since then given age (unstable)",
            )
            .hide(true)
            .value_name("DURATION"),
        )
        .arg(
            opt(
                "max-git-db-age",
                "Deletes git dependency clones that have not been used since then given age (unstable)",
            )
            .hide(true)
            .value_name("DURATION"),
        )
        .arg(
            opt(
                "max-download-age",
                "Deletes any downloaded cache data that has not been used since then given age (unstable)",
            )
            .hide(true)
            .value_name("DURATION"),
        )

        .arg(
            opt(
                "max-src-size",
                "Deletes source cache files until the cache is under the given size (unstable)",
            )
            .hide(true)
            .value_name("SIZE"),
        )
        .arg(
            opt(
                "max-crate-size",
                "Deletes crate cache files until the cache is under the given size (unstable)",
            )
            .hide(true)
            .value_name("SIZE"),
        )
        .arg(
            opt("max-git-size",
                "Deletes git dependency caches until the cache is under the given size (unstable")
            .hide(true)
            .value_name("SIZE"))
        .arg(
            opt(
                "max-download-size",
                "Deletes downloaded cache data until the cache is under the given size (unstable)",
            )
            .hide(true)
            .value_name("DURATION"),
        )

        // These are unimplemented. Leaving here as a guide for how this is
        // intended to evolve. These will likely change, this is just a sketch
        // of ideas.
        .arg(
            opt(
                "max-target-age",
                "Deletes any build artifact files that have not been used since then given age (unstable) (UNIMPLEMENTED)",
            )
            .hide(true)
            .value_name("DURATION"),
        )
        .arg(
            // TODO: come up with something less wordy?
            opt(
                "max-shared-target-age",
                "Deletes any shared build artifact files that have not been used since then given age (unstable) (UNIMPLEMENTED)",
            )
            .hide(true)
            .value_name("DURATION"),
        )
        .arg(
            opt(
                "max-target-size",
                "Deletes build artifact files until the cache is under the given size (unstable) (UNIMPLEMENTED)",
            )
            .hide(true)
            .value_name("SIZE"),
        )
        .arg(
            // TODO: come up with something less wordy?
            opt(
                "max-shared-target-size",
                "Deletes shared build artifact files until the cache is under the given size (unstable) (UNIMPLEMENTED)",
            )
            .hide(true)
            .value_name("DURATION"),
        )

        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help clean</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config);

    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws?)?;
        return Ok(());
    }

    let unstable_gc = |opt| {
        // TODO: issue number
        config
            .cli_unstable()
            .fail_if_stable_opt_custom_z(opt, 0, "gc", config.cli_unstable().gc)
    };
    let unstable_cache_opt = |opt| -> CargoResult<Option<&str>> {
        let arg = args.get_one::<String>(opt).map(String::as_str);
        if arg.is_some() {
            unstable_gc(opt)?;
        }
        Ok(arg)
    };
    let unstable_size_opt = |opt| -> CargoResult<Option<u64>> {
        unstable_cache_opt(opt)?
            .map(|s| parse_human_size(s))
            .transpose()
    };
    let unstable_duration_opt = |opt| -> CargoResult<Option<Duration>> {
        unstable_cache_opt(opt)?
            .map(|s| parse_time_span(s))
            .transpose()
    };
    let unimplemented_opt = |opt| -> CargoResult<Option<&str>> {
        let arg = args.get_one::<String>(opt).map(String::as_str);
        if arg.is_some() {
            anyhow::bail!("option --{opt} is not yet implemented");
        }
        Ok(None)
    };
    let unimplemented_size_opt = |opt| -> CargoResult<Option<u64>> {
        unimplemented_opt(opt)?;
        Ok(None)
    };
    let unimplemented_duration_opt = |opt| -> CargoResult<Option<Duration>> {
        unimplemented_opt(opt)?;
        Ok(None)
    };

    let mut gc: Vec<_> = args
        .get_many::<AutoGcKind>("gc")
        .unwrap_or_default()
        .cloned()
        .collect();
    if gc.is_empty() && args.contains_id("gc") {
        gc.push(AutoGcKind::All);
    }
    if !gc.is_empty() {
        unstable_gc("gc")?;
    }

    let mut gc_opts = GcOpts {
        max_src_age: unstable_duration_opt("max-src-age")?,
        max_crate_age: unstable_duration_opt("max-crate-age")?,
        max_index_age: unstable_duration_opt("max-index-age")?,
        max_git_co_age: unstable_duration_opt("max-git-co-age")?,
        max_git_db_age: unstable_duration_opt("max-git-db-age")?,
        max_src_size: unstable_size_opt("max-src-size")?,
        max_crate_size: unstable_size_opt("max-crate-size")?,
        max_git_size: unstable_size_opt("max-git-size")?,
        max_download_size: unstable_size_opt("max-download-size")?,
        max_target_age: unimplemented_duration_opt("max-target-age")?,
        max_shared_target_age: unimplemented_duration_opt("max-shared-target-age")?,
        max_target_size: unimplemented_size_opt("max-target-size")?,
        max_shared_target_size: unimplemented_size_opt("max-shared-target-size")?,
    };
    let max_download_age = unstable_duration_opt("max-download-age")?;
    gc_opts.update_for_auto_gc(config, &gc, max_download_age)?;

    let opts = CleanOptions {
        config,
        spec: values(args, "package"),
        targets: args.targets()?,
        requested_profile: args.get_profile_name(config, "dev", ProfileChecking::Custom)?,
        profile_specified: args.contains_id("profile") || args.flag("release"),
        doc: args.flag("doc"),
        dry_run: args.dry_run(),
        gc_opts,
    };
    ops::clean(ws, &opts)?;
    Ok(())
}
