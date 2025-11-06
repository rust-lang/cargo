use crate::command_prelude::*;

use cargo::{CargoResult, core::PackageId, ops};
use clap_complete::ArgValueCandidates;

use std::collections::BTreeSet;

pub fn cli() -> Command {
    subcommand("uninstall")
        .about("Remove a Rust binary")
        .arg(
            Arg::new("spec")
                .value_name("SPEC")
                .num_args(0..)
                .add::<clap_complete::ArgValueCandidates>(clap_complete::ArgValueCandidates::new(
                    || get_installed_crates(),
                )),
        )
        .arg(opt("root", "Directory to uninstall packages from").value_name("DIR"))
        .arg_silent_suggestion()
        .arg_package_spec_simple(
            "Package to uninstall",
            ArgValueCandidates::new(get_installed_package_candidates),
        )
        .arg(
            multi_opt("bin", "NAME", "Only uninstall the binary NAME")
                .help_heading(heading::TARGET_SELECTION),
        )
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help uninstall</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let root = args.get_one::<String>("root").map(String::as_str);

    if args.is_present_with_zero_values("package") {
        return Err(anyhow::anyhow!(
            "\"--package <SPEC>\" requires a SPEC format value.\n\
            Run `cargo help pkgid` for more information about SPEC format."
        )
        .into());
    }

    let specs = args
        .get_many::<String>("spec")
        .unwrap_or_else(|| args.get_many::<String>("package").unwrap_or_default())
        .map(String::as_str)
        .collect();
    ops::uninstall(root, specs, &values(args, "bin"), gctx)?;
    Ok(())
}

fn get_installed_crates() -> Vec<clap_complete::CompletionCandidate> {
    get_installed_crates_().unwrap_or_default()
}

fn get_installed_crates_() -> Option<Vec<clap_complete::CompletionCandidate>> {
    let mut candidates = Vec::new();

    let gctx = new_gctx_for_completions().ok()?;

    let root = ops::resolve_root(None, &gctx).ok()?;

    let tracker = ops::InstallTracker::load(&gctx, &root).ok()?;

    for (_, v) in tracker.all_installed_bins() {
        for bin in v {
            candidates.push(clap_complete::CompletionCandidate::new(bin));
        }
    }

    Some(candidates)
}

fn get_installed_package_candidates() -> Vec<clap_complete::CompletionCandidate> {
    get_installed_packages()
        .unwrap_or_default()
        .into_iter()
        .map(|(pkg, bins)| {
            let single_binary = bins.iter().next().take_if(|_| bins.len() == 1);

            let help = if single_binary.is_some_and(|bin| bin == pkg.name().as_str()) {
                None
            } else {
                let binaries = bins.into_iter().collect::<Vec<_>>().as_slice().join(", ");
                Some(binaries)
            };

            clap_complete::CompletionCandidate::new(pkg.name().as_str()).help(help.map(From::from))
        })
        .collect()
}

fn get_installed_packages() -> CargoResult<Vec<(PackageId, BTreeSet<String>)>> {
    let gctx = new_gctx_for_completions()?;
    let root = ops::resolve_root(None, &gctx)?;

    let tracker = ops::InstallTracker::load(&gctx, &root)?;
    Ok(tracker
        .all_installed_bins()
        .map(|(package_id, bins)| (*package_id, bins.clone()))
        .collect())
}
