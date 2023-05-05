use cargo::core::registry::PackageRegistry;
use cargo::core::QueryKind;
use cargo::core::Registry;
use cargo::core::SourceId;
use cargo::util::command_prelude::*;
use cargo::util::interning::InternedString;
use cargo::CargoResult;
use termcolor::Color;
use termcolor::ColorSpec;

const UPSTREAM_BRANCH: &str = "master";

pub fn cli() -> clap::Command {
    clap::Command::new("xtask-changes")
        .arg(clap::Arg::new("base-sha").help("SHA to diff against"))
        .arg(clap::Arg::new("head-sha").help("SHA with changes"))
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

    changes(args, config)?;

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

fn changes(args: &clap::ArgMatches, config: &mut cargo::util::Config) -> CargoResult<()> {
    let repo = git2::Repository::discover(".")?;

    let base_commit = match args.get_one::<String>("base-sha") {
        Some(sha) => {
            let head_obj = repo.revparse_single(sha)?;
            head_obj.peel_to_commit()?
        }
        None => {
            let branches = repo
                .branches(Some(git2::BranchType::Remote))?
                .collect::<Vec<_>>();
            log::trace!(
                "branches found: {:?}",
                branches
                    .iter()
                    .filter_map(|r| r.as_ref().ok())
                    .map(|(b, _)| b.name())
                    .collect::<Vec<_>>()
            );
            let upstream_branches = branches
                .into_iter()
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
            log::trace!(
                "upstream branches found: {:?}",
                upstream_branches
                    .iter()
                    .map(|b| b.name())
                    .collect::<Vec<_>>()
            );
            if upstream_branches.is_empty() {
                anyhow::bail!(
                    "could not find `base-sha` for `{UPSTREAM_BRANCH}`, pass it in directly"
                );
            }
            let upstream_ref = upstream_branches[0].get();
            if 1 < upstream_branches.len() {
                let name = upstream_ref.name().expect("name is valid UTF-8");
                let _ = config.shell().warn(format!(
                    "multiple `{UPSTREAM_BRANCH}` found, picking {name}"
                ));
            }
            upstream_ref.peel_to_commit()?
        }
    };
    let head_commit = match args.get_one::<String>("head-sha") {
        Some(sha) => {
            let head_obj = repo.revparse_single(sha)?;
            head_obj.peel_to_commit()?
        }
        None => {
            let head_ref = repo.head()?;
            head_ref.peel_to_commit()?
        }
    };

    let base_id = base_commit.id();
    let head_id = head_commit.id();
    if base_id == head_id {
        let _ = config.shell().warn(format!(
            "no changes; commit range is empty ({base_id}..{head_id})"
        ));
        return Ok(());
    }

    let commits = CommitDescription::walk(&repo, base_id, head_id, config)?;

    let ws = args.workspace(config)?;
    let mut results = Vec::new();
    {
        let mut registry = PackageRegistry::new(config)?;
        let _lock = config.acquire_package_cache_lock()?;
        registry.lock_patches();
        let source_id = SourceId::crates_io(config)?;

        for member in ws.members() {
            let name = member.name();
            let local = member.version().clone();
            if member.publish() == &Some(vec![]) {
                log::trace!("skipping {name}, `publish = false`");
                continue;
            }
            let _ = config
                .shell()
                .status("Analyzing", format!("{name} {local}"));

            let commits = commits
                .iter()
                .filter(|commit| commit.paths.iter().any(|p| p.starts_with(member.root())))
                .filter(|commit| {
                    for other in ws.members() {
                        if member.manifest_path() == other.manifest_path() {
                            continue;
                        }
                        if !other.root().starts_with(member.root()) {
                            continue;
                        }
                        if commit.paths.iter().any(|p| p.starts_with(other.root())) {
                            // more specific member exists
                            return false;
                        }
                    }
                    true
                })
                .cloned()
                .collect::<Vec<_>>();
            if commits.is_empty() {
                continue;
            }

            let version_req = format!("<={local}");
            let query = cargo::core::dependency::Dependency::parse(
                name,
                Some(&version_req),
                source_id.clone(),
            )?;
            let possibilities = loop {
                // Exact to avoid returning all for path/git
                match registry.query_vec(&query, QueryKind::Exact) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            };
            let published = possibilities.iter().map(|s| s.version()).max().cloned();

            results.push(ChangedPackage {
                name,
                local,
                published,
                commits,
            });
        }
    }

    for changed in &results {
        let name = &changed.name;
        let local = &changed.local;
        let published = changed
            .published
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_owned());
        let _ = config.shell().status(
            "Changed",
            format!("{name} {local} was last published as {published}"),
        );
        let prefix = format!("{:>13}", " ");
        for commit in &commits {
            let _ = config.shell().write_stderr(&prefix, &ColorSpec::new());
            let _ = config.shell().write_stderr(
                &commit.short_id,
                ColorSpec::new().set_fg(Some(Color::Yellow)),
            );
            let _ = config.shell().write_stderr(" ", &ColorSpec::new());
            let _ = config
                .shell()
                .write_stderr(&commit.summary, &ColorSpec::new());

            let current_status = commit.status();
            write_status(current_status, config);
            let _ = config.shell().write_stderr("\n", &ColorSpec::new());
        }
    }

    Ok(())
}

fn write_status(status: Option<CommitStatus>, config: &cargo::util::Config) {
    if let Some(status) = status {
        let suffix;
        let mut color = ColorSpec::new();
        match status {
            CommitStatus::Breaking => {
                suffix = format!(" ({})", status);
                color.set_fg(Some(Color::Red));
            }
            CommitStatus::Feature => {
                suffix = format!(" ({})", status);
                color.set_fg(Some(Color::Yellow));
            }
            CommitStatus::Fix => {
                suffix = format!(" ({})", status);
                color.set_fg(Some(Color::Green));
            }
            CommitStatus::Ignore => {
                suffix = String::new();
            }
        }
        let _ = config.shell().write_stderr(suffix, &color);
    }
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct ChangedPackage {
    name: InternedString,
    local: semver::Version,
    published: Option<semver::Version>,
    commits: Vec<CommitDescription>,
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct CommitDescription {
    id: git2::Oid,
    short_id: String,
    summary: String,
    message: String,
    paths: std::collections::BTreeSet<std::path::PathBuf>,
}

impl CommitDescription {
    fn walk(
        repo: &git2::Repository,
        base_id: git2::Oid,
        head_id: git2::Oid,
        config: &cargo::util::Config,
    ) -> CargoResult<Vec<Self>> {
        let repo_path = repo
            .workdir()
            .ok_or_else(|| anyhow::format_err!("bare repos are unsupported"))?;

        let mut commits = Vec::new();
        let mut revwalk = repo.revwalk()?;
        let range = format!("{base_id}..{head_id}");
        let _ = config.shell().status("Walking", range.clone());
        revwalk.push_range(&range)?;
        for commit_id in revwalk {
            let commit_id = commit_id?;
            let commit = repo.find_commit(commit_id)?;
            if 1 < commit.parent_count() {
                log::trace!("assuming merge commits ({}) can be ignored", commit.id());
                continue;
            }
            if commit.parent_count() == 0 {
                log::trace!("assuming initial commits ({}) can be ignored", commit.id());
                continue;
            }

            let parent_tree = commit.parent(0).ok().map(|c| c.tree()).transpose()?;
            let tree = commit.tree()?;
            let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

            let mut changed_paths = std::collections::BTreeSet::new();
            for delta in diff.deltas() {
                if let Some(old_path) = delta.old_file().path() {
                    changed_paths.insert(repo_path.join(old_path));
                }
                if let Some(new_path) = delta.new_file().path() {
                    changed_paths.insert(repo_path.join(new_path));
                }
            }

            if !changed_paths.is_empty() {
                let short_id =
                    String::from_utf8_lossy(&repo.find_object(commit_id, None)?.short_id()?)
                        .into_owned();
                commits.push(CommitDescription {
                    id: commit_id,
                    short_id,
                    summary: String::from_utf8_lossy(commit.summary_bytes().unwrap_or(b""))
                        .into_owned(),
                    message: String::from_utf8_lossy(commit.message_bytes()).into_owned(),
                    paths: changed_paths,
                });
            }
        }
        Ok(commits)
    }

    fn status(&self) -> Option<CommitStatus> {
        if let Some(status) = self.conventional_status() {
            return status;
        }

        None
    }

    fn conventional_status(&self) -> Option<Option<CommitStatus>> {
        let parts = git_conventional::Commit::parse(&self.message).ok()?;
        if parts.breaking() {
            return Some(Some(CommitStatus::Breaking));
        }

        if [
            git_conventional::Type::CHORE,
            git_conventional::Type::TEST,
            git_conventional::Type::STYLE,
            git_conventional::Type::REFACTOR,
            git_conventional::Type::REVERT,
        ]
        .contains(&parts.type_())
        {
            Some(Some(CommitStatus::Ignore))
        } else if [
            git_conventional::Type::DOCS,
            git_conventional::Type::PERF,
            git_conventional::Type::FIX,
        ]
        .contains(&parts.type_())
        {
            Some(Some(CommitStatus::Fix))
        } else if [git_conventional::Type::FEAT].contains(&parts.type_()) {
            Some(Some(CommitStatus::Feature))
        } else {
            Some(None)
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommitStatus {
    Ignore,
    Fix,
    Feature,
    Breaking,
}

impl CommitStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::Ignore => "ignore",
            Self::Fix => "fix",
            Self::Feature => "feature",
            Self::Breaking => "breaking",
        }
    }
}

impl std::fmt::Display for CommitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

#[test]
fn verify_cli() {
    cli().debug_assert();
}
