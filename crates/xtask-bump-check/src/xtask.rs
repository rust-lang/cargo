//! ```text
//! NAME
//!         xtask-bump-check
//!
//! SYNOPSIS
//!         xtask-bump-check --base-rev <REV> --head-rev <REV>
//!
//! DESCRIPTION
//!         Checks if there is any member got changed since a base commit
//!         but forgot to bump its version.
//! ```

use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::task;

use cargo::core::dependency::Dependency;
use cargo::core::registry::PackageRegistry;
use cargo::core::Package;
use cargo::core::Registry;
use cargo::core::SourceId;
use cargo::core::Workspace;
use cargo::sources::source::QueryKind;
use cargo::util::cache_lock::CacheLockMode;
use cargo::util::command_prelude::*;
use cargo::util::ToSemver;
use cargo::CargoResult;
use cargo_util::ProcessBuilder;

const UPSTREAM_BRANCH: &str = "master";
const STATUS: &str = "BumpCheck";

pub fn cli() -> clap::Command {
    clap::Command::new("xtask-bump-check")
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
        .arg(opt("base-rev", "Git revision to lookup for a baseline"))
        .arg(opt("head-rev", "Git revision with changes"))
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

    bump_check(args, config)?;

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

/// Main entry of `xtask-bump-check`.
///
/// Assumption: version number are incremental. We never have point release for old versions.
fn bump_check(args: &clap::ArgMatches, config: &cargo::util::Config) -> CargoResult<()> {
    let ws = args.workspace(config)?;
    let repo = git2::Repository::open(ws.root())?;
    let base_commit = get_base_commit(config, args, &repo)?;
    let head_commit = get_head_commit(args, &repo)?;
    let referenced_commit = get_referenced_commit(&repo, &base_commit)?;
    let changed_members = changed(&ws, &repo, &base_commit, &head_commit)?;
    let status = |msg: &str| config.shell().status(STATUS, msg);

    status(&format!("base commit `{}`", base_commit.id()))?;
    status(&format!("head commit `{}`", head_commit.id()))?;

    let mut needs_bump = Vec::new();

    check_crates_io(config, &changed_members, &mut needs_bump)?;

    if let Some(referenced_commit) = referenced_commit.as_ref() {
        status(&format!("compare against `{}`", referenced_commit.id()))?;
        for referenced_member in checkout_ws(&ws, &repo, referenced_commit)?.members() {
            let pkg_name = referenced_member.name().as_str();
            let Some(changed_member) = changed_members.get(pkg_name) else {
                tracing::trace!("skipping {pkg_name}, may be removed or not published");
                continue;
            };

            if changed_member.version() <= referenced_member.version() {
                needs_bump.push(*changed_member);
            }
        }
    }

    if !needs_bump.is_empty() {
        needs_bump.sort();
        needs_bump.dedup();
        let mut msg = String::new();
        msg.push_str("Detected changes in these crates but no version bump found:\n");
        for pkg in needs_bump {
            writeln!(&mut msg, "  {}@{}", pkg.name(), pkg.version())?;
        }
        msg.push_str("\nPlease bump at least one patch version in each corresponding Cargo.toml.");
        anyhow::bail!(msg)
    }

    // Even when we test against baseline-rev, we still need to make sure a
    // change doesn't violate SemVer rules against crates.io releases. The
    // possibility of this happening is nearly zero but no harm to check twice.
    let mut cmd = ProcessBuilder::new("cargo");
    cmd.arg("semver-checks")
        .arg("check-release")
        .arg("--workspace");
    config.shell().status("Running", &cmd)?;
    cmd.exec()?;

    if let Some(referenced_commit) = referenced_commit.as_ref() {
        let mut cmd = ProcessBuilder::new("cargo");
        cmd.arg("semver-checks")
            .arg("--workspace")
            .arg("--baseline-rev")
            .arg(referenced_commit.id().to_string());
        config.shell().status("Running", &cmd)?;
        cmd.exec()?;
    }

    status("no version bump needed for member crates.")?;

    Ok(())
}

/// Returns the commit of upstream `master` branch if `base-rev` is missing.
fn get_base_commit<'a>(
    config: &Config,
    args: &clap::ArgMatches,
    repo: &'a git2::Repository,
) -> CargoResult<git2::Commit<'a>> {
    let base_commit = match args.get_one::<String>("base-rev") {
        Some(sha) => {
            let obj = repo.revparse_single(sha)?;
            obj.peel_to_commit()?
        }
        None => {
            let upstream_branches = repo
                .branches(Some(git2::BranchType::Remote))?
                .filter_map(|r| r.ok())
                .filter(|(b, _)| {
                    b.name()
                        .ok()
                        .flatten()
                        .unwrap_or_default()
                        .ends_with(&format!("/{UPSTREAM_BRANCH}"))
                })
                .map(|(b, _)| b)
                .collect::<Vec<_>>();
            if upstream_branches.is_empty() {
                anyhow::bail!(
                    "could not find `base-sha` for `{UPSTREAM_BRANCH}`, pass it in directly"
                );
            }
            let upstream_ref = upstream_branches[0].get();
            if upstream_branches.len() > 1 {
                let name = upstream_ref.name().expect("name is valid UTF-8");
                let _ = config.shell().warn(format!(
                    "multiple `{UPSTREAM_BRANCH}` found, picking {name}"
                ));
            }
            upstream_ref.peel_to_commit()?
        }
    };
    Ok(base_commit)
}

/// Returns `HEAD` of the Git repository if `head-rev` is missing.
fn get_head_commit<'a>(
    args: &clap::ArgMatches,
    repo: &'a git2::Repository,
) -> CargoResult<git2::Commit<'a>> {
    let head_commit = match args.get_one::<String>("head-rev") {
        Some(sha) => {
            let head_obj = repo.revparse_single(sha)?;
            head_obj.peel_to_commit()?
        }
        None => {
            let head_ref = repo.head()?;
            head_ref.peel_to_commit()?
        }
    };
    Ok(head_commit)
}

/// Gets the referenced commit to compare if version bump needed.
///
/// * When merging into nightly, check the version with beta branch
/// * When merging into beta, check the version with stable branch
/// * When merging into stable, check against crates.io registry directly
fn get_referenced_commit<'a>(
    repo: &'a git2::Repository,
    base: &git2::Commit<'a>,
) -> CargoResult<Option<git2::Commit<'a>>> {
    let [beta, stable] = beta_and_stable_branch(repo)?;
    let rev_id = base.id();
    let stable_commit = stable.get().peel_to_commit()?;
    let beta_commit = beta.get().peel_to_commit()?;

    let referenced_commit = if rev_id == stable_commit.id() {
        None
    } else if rev_id == beta_commit.id() {
        tracing::trace!("stable branch from `{}`", stable.name().unwrap().unwrap());
        Some(stable_commit)
    } else {
        tracing::trace!("beta branch from `{}`", beta.name().unwrap().unwrap());
        Some(beta_commit)
    };

    Ok(referenced_commit)
}

/// Get the current beta and stable branch in cargo repository.
///
/// Assumptions:
///
/// * The repository contains the full history of `<remote>/rust-1.*.0` branches.
/// * The version part of `<remote>/rust-1.*.0` always ends with a zero.
/// * The maximum version is for beta channel, and the second one is for stable.
fn beta_and_stable_branch(repo: &git2::Repository) -> CargoResult<[git2::Branch<'_>; 2]> {
    let mut release_branches = Vec::new();
    for branch in repo.branches(Some(git2::BranchType::Remote))? {
        let (branch, _) = branch?;
        let name = branch.name()?.unwrap();
        let Some((_, version)) = name.split_once("/rust-") else {
            tracing::trace!("branch `{name}` is not in the format of `<remote>/rust-<semver>`");
            continue;
        };
        let Ok(version) = version.to_semver() else {
            tracing::trace!("branch `{name}` is not a valid semver: `{version}`");
            continue;
        };
        release_branches.push((version, branch));
    }
    release_branches.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    release_branches.dedup_by(|a, b| a.0 == b.0);

    let beta = release_branches.pop().unwrap();
    let stable = release_branches.pop().unwrap();

    assert_eq!(beta.0.major, 1);
    assert_eq!(beta.0.patch, 0);
    assert_eq!(stable.0.major, 1);
    assert_eq!(stable.0.patch, 0);
    assert_ne!(beta.0.minor, stable.0.minor);

    Ok([beta.1, stable.1])
}

/// Lists all changed workspace members between two commits.
fn changed<'r, 'ws>(
    ws: &'ws Workspace<'_>,
    repo: &'r git2::Repository,
    base_commit: &git2::Commit<'r>,
    head: &git2::Commit<'r>,
) -> CargoResult<HashMap<&'ws str, &'ws Package>> {
    let root_pkg_name = ws.current()?.name(); // `cargo` crate.
    let ws_members = ws
        .members()
        .filter(|pkg| pkg.name() != root_pkg_name) // Only take care of sub crates here.
        .filter(|pkg| pkg.publish() != &Some(vec![])) // filter out `publish = false`
        .map(|pkg| {
            // Having relative package root path so that we can compare with
            // paths of changed files to determine which package has changed.
            let relative_pkg_root = pkg.root().strip_prefix(ws.root()).unwrap();
            (relative_pkg_root, pkg)
        })
        .collect::<Vec<_>>();
    let base_tree = base_commit.as_object().peel_to_tree()?;
    let head_tree = head.as_object().peel_to_tree()?;
    let diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Default::default())?;

    let mut changed_members = HashMap::new();

    for delta in diff.deltas() {
        let old = delta.old_file().path().unwrap();
        let new = delta.new_file().path().unwrap();
        for (ref pkg_root, pkg) in ws_members.iter() {
            if old.starts_with(pkg_root) || new.starts_with(pkg_root) {
                changed_members.insert(pkg.name().as_str(), *pkg);
                break;
            }
        }
    }

    tracing::trace!("changed_members: {:?}", changed_members.keys());
    Ok(changed_members)
}

/// Compares version against published crates on crates.io.
///
/// Assumption: We always release a version larger than all existing versions.
fn check_crates_io<'a>(
    config: &Config,
    changed_members: &HashMap<&'a str, &'a Package>,
    needs_bump: &mut Vec<&'a Package>,
) -> CargoResult<()> {
    let source_id = SourceId::crates_io(config)?;
    let mut registry = PackageRegistry::new(config)?;
    let _lock = config.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
    registry.lock_patches();
    config.shell().status(
        STATUS,
        format_args!("compare against `{}`", source_id.display_registry_name()),
    )?;
    for (name, member) in changed_members {
        let current = member.version();
        let version_req = format!(">={current}");
        let query = Dependency::parse(*name, Some(&version_req), source_id)?;
        let possibilities = loop {
            // Exact to avoid returning all for path/git
            match registry.query_vec(&query, QueryKind::Exact) {
                task::Poll::Ready(res) => {
                    break res?;
                }
                task::Poll::Pending => registry.block_until_ready()?,
            }
        };
        if possibilities.is_empty() {
            tracing::trace!("dep `{name}` has no version greater than or equal to `{current}`");
        } else {
            tracing::trace!(
                "`{name}@{current}` needs a bump because its should have a version newer than crates.io: {:?}`",
                possibilities
                    .iter()
                    .map(|s| format!("{}@{}", s.name(), s.version()))
                    .collect::<Vec<_>>(),
            );
            needs_bump.push(member);
        }
    }

    Ok(())
}

/// Checkouts a temporary workspace to do further version comparisons.
fn checkout_ws<'cfg, 'a>(
    ws: &Workspace<'cfg>,
    repo: &'a git2::Repository,
    referenced_commit: &git2::Commit<'a>,
) -> CargoResult<Workspace<'cfg>> {
    let repo_path = repo.path().as_os_str().to_str().unwrap();
    // Put it under `target/cargo-<short-id>`
    let short_id = &referenced_commit.id().to_string()[..7];
    let checkout_path = ws.target_dir().join(format!("cargo-{short_id}"));
    let checkout_path = checkout_path.as_path_unlocked();
    let _ = fs::remove_dir_all(checkout_path);
    let new_repo = git2::build::RepoBuilder::new()
        .clone_local(git2::build::CloneLocal::Local)
        .clone(repo_path, checkout_path)?;
    let obj = new_repo.find_object(referenced_commit.id(), None)?;
    new_repo.reset(&obj, git2::ResetType::Hard, None)?;
    Workspace::new(&checkout_path.join("Cargo.toml"), ws.config())
}

#[test]
fn verify_cli() {
    cli().debug_assert();
}
