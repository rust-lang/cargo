use crate::core::{Package, Workspace};
use crate::ops::cargo_package::vcs::{
    dirty_files_outside_pkg_root, status_submodules, GitVcsInfo, VcsInfo,
};
use crate::ops::PackageOpts;
use crate::sources::PathEntry;
use crate::{CargoResult, GlobalContext};
use anyhow::Context;
use cargo_util::paths;
use gix::bstr::ByteSlice;
use gix::diff::rewrites::tracker::Change;
use gix::dir::walk::EmissionMode;
use gix::index::entry::Mode;
use gix::status::tree_index::TrackRenames;
use gix::worktree::stack::state::ignore::Source;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Checks if the package source is in a *git* DVCS repository.
///
/// If *git*, and the source is *dirty* (e.g., has uncommitted changes),
/// and `--allow-dirty` has not been passed,
/// then `bail!` with an informative message.
/// Otherwise, return the sha1 hash of the current *HEAD* commit,
/// or `None` if no repo is found.
#[tracing::instrument(skip_all)]
pub fn check_repo_state(
    p: &Package,
    src_files: &[PathEntry],
    ws: &Workspace<'_>,
    opts: &PackageOpts<'_>,
) -> CargoResult<Option<VcsInfo>> {
    let gctx = ws.gctx();
    let Ok(git2_repo) = git2::Repository::discover(p.root()) else {
        gctx.shell().verbose(|shell| {
            shell.warn(format_args!(
                "no (git) VCS found for `{}`",
                p.root().display()
            ))
        })?;
        // No Git repo found. Have to assume it is clean.
        return Ok(None);
    };
    let Ok(repo) = gix::discover(p.root()) else {
        gctx.shell().verbose(|shell| {
            shell.warn(format_args!(
                "no (git) VCS found for `{}`",
                p.root().display()
            ))
        })?;
        // No Git repo found. Have to assume it is clean.
        return Ok(None);
    };

    let Some(workdir) = repo.workdir() else {
        debug!(
            "no (git) workdir found for repo at `{}`",
            repo.path().display()
        );
        // No git workdir. Have to assume it is clean.
        return Ok(None);
    };

    debug!("found a git repo at `{}`", workdir.display());
    let path = p.manifest_path();

    let manifest_exists = path.exists();
    let path = paths::strip_prefix_canonical(path, workdir).unwrap_or_else(|_| path.to_path_buf());
    let rela_path =
        gix::path::to_unix_separators_on_windows(gix::path::os_str_into_bstr(path.as_os_str())?);
    if !manifest_exists {
        gctx.shell().verbose(|shell| {
            shell.warn(format_args!(
                "Cargo.toml not found at `{}` in workdir `{}`",
                path.display(),
                workdir.display()
            ))
        })?;
        // TODO: Either remove this whole block, or have a test.
        //       It's hard to have no Cargo.toml here?
        // No `Cargo.toml` found. This package may be irrelevant.
        // Have to assume it is clean.
        return Ok(None);
    };

    let manifest_is_ignored = {
        let index = repo.index_or_empty()?;
        let mut excludes =
            repo.excludes(&index, None, Source::WorktreeThenIdMappingIfNotSkipped)?;
        excludes
            .at_entry(rela_path.as_bstr(), Some(Mode::FILE))?
            .is_excluded()
    };
    if manifest_is_ignored {
        gctx.shell().verbose(|shell| {
            shell.warn(format_args!(
                "found (git) Cargo.toml ignored at `{}` in workdir `{}`",
                path.display(),
                workdir.display()
            ))
        })?;
        // An ignored `Cargo.toml` found. This package may be irrelevant.
        // Have to assume it is clean.
        return Ok(None);
    }

    warn_symlink_checked_out_as_plain_text_file(gctx, src_files, &repo)?;

    debug!(
        "found (git) Cargo.toml at `{}` in workdir `{}`",
        path.display(),
        workdir.display(),
    );
    let Some(git) = git(ws, p, src_files, &repo, &git2_repo, &opts)? else {
        // If the git repo lacks essential field like `sha1`, and since this field exists from the beginning,
        // then don't generate the corresponding file in order to maintain consistency with past behavior.
        return Ok(None);
    };

    let path_in_vcs = path
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .replace("\\", "/");

    Ok(Some(VcsInfo { git, path_in_vcs }))
}

/// Warns if any symlinks were checked out as plain text files.
///
/// Git config [`core.symlinks`] defaults to true when unset.
/// In git-for-windows (and git as well),
/// the config should be set to false explicitly when the repo was created,
/// if symlink support wasn't detected [^1].
///
/// We assume the config was always set at creation time and never changed.
/// So, if it is true, we don't bother users with any warning.
///
/// [^1]: <https://github.com/git-for-windows/git/blob/f1241afcc7956918d5da33ef74abd9cbba369247/setup.c#L2394-L2403>
///
/// [`core.symlinks`]: https://git-scm.com/docs/git-config#Documentation/git-config.txt-coresymlinks
fn warn_symlink_checked_out_as_plain_text_file(
    gctx: &GlobalContext,
    src_files: &[PathEntry],
    repo: &gix::Repository,
) -> CargoResult<()> {
    if repo
        .config_snapshot()
        .boolean(&gix::config::tree::Core::SYMLINKS)
        .unwrap_or(true)
    {
        return Ok(());
    }

    if src_files.iter().any(|f| f.maybe_plain_text_symlink()) {
        let mut shell = gctx.shell();
        shell.warn(format_args!(
            "found symbolic links that may be checked out as regular files for git repo at `{}/`\n\
        This might cause the `.crate` file to include incorrect or incomplete files",
            repo.workdir().unwrap().display(),
        ))?;
        let extra_note = if cfg!(windows) {
            "\nAnd on Windows, enable the Developer Mode to support symlinks"
        } else {
            ""
        };
        shell.note(format_args!(
            "to avoid this, set the Git config `core.symlinks` to `true`{extra_note}",
        ))?;
    }

    Ok(())
}

/// The real git status check starts from here.
fn git(
    ws: &Workspace<'_>,
    pkg: &Package,
    src_files: &[PathEntry],
    repo: &gix::Repository,
    git_repo: &git2::Repository,
    opts: &PackageOpts<'_>,
) -> CargoResult<Option<GitVcsInfo>> {
    // This is a collection of any dirty or untracked files. This covers:
    // - new/modified/deleted/renamed/type change (index or worktree)
    // - untracked files (which are "new" worktree files)
    // - ignored (in case the user has an `include` directive that
    //   conflicts with .gitignore).
    let mut dirty_files = Vec::new();
    collect_statuses(
        repo,
        relative_package_root(repo, pkg.root()).as_deref(),
        &mut dirty_files,
    )?;
    // super::collect_statuses(git_repo, &[pathspec.as_str()], &mut dirty_files)?;

    // Include each submodule so that the error message can provide
    // specifically *which* files in a submodule are modified.
    status_submodules(git_repo, &mut dirty_files)?;

    // Find the intersection of dirty in git, and the src_files that would
    // be packaged. This is a lazy n^2 check, but seems fine with
    // thousands of files.
    let cwd = ws.gctx().cwd();
    let mut dirty_src_files: Vec<_> = src_files
        .iter()
        .filter(|src_file| dirty_files.iter().any(|path| src_file.starts_with(path)))
        .map(|p| p.as_ref())
        .chain(dirty_files_outside_pkg_root(ws, pkg, git_repo, src_files)?.iter())
        .map(|path| {
            pathdiff::diff_paths(path, cwd)
                .as_ref()
                .unwrap_or(path)
                .display()
                .to_string()
        })
        .collect();
    let dirty = !dirty_src_files.is_empty();
    if !dirty || opts.allow_dirty {
        // Must check whetherthe repo has no commit firstly, otherwise `revparse_single` would fail on bare commit repo.
        // Due to lacking the `sha1` field, it's better not record the `GitVcsInfo` for consistency.
        if git_repo.is_empty()? {
            return Ok(None);
        }
        let rev_obj = git_repo.revparse_single("HEAD")?;
        Ok(Some(GitVcsInfo {
            sha1: rev_obj.id().to_string(),
            dirty,
        }))
    } else {
        dirty_src_files.sort_unstable();
        anyhow::bail!(
            "{} files in the working directory contain changes that were \
             not yet committed into git:\n\n{}\n\n\
             to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag",
            dirty_src_files.len(),
            dirty_src_files.join("\n")
        )
    }
}

/// Helper to collect dirty statuses for a single repo.
/// `relative_package_root` is `Some` if the root is a sub-directory of the workdir.
fn collect_statuses(
    repo: &gix::Repository,
    relative_package_root: Option<&Path>,
    dirty_files: &mut Vec<PathBuf>,
) -> CargoResult<()> {
    let statuses = repo
        .status(gix::progress::Discard)?
        .dirwalk_options(|opts| {
            opts.emit_untracked(gix::dir::walk::EmissionMode::Matching)
                // Also pick up ignored files (but not entire directories)
                // to specifically catch overzealously ignored source files.
                // Later we will match these dirs by prefix.
                // TODO: make sure dirs have a trailing separator
                .emit_ignored(Some(EmissionMode::CollapseDirectory))
                .emit_tracked(false)
                .recurse_repositories(false)
                .symlinks_to_directories_are_ignored_like_directories(true)
                .emit_empty_directories(false)
        })
        .tree_index_track_renames(TrackRenames::Disabled)
        .index_worktree_submodules(None)
        .into_iter(
            relative_package_root
                .map(|rela_root| {
                    gix::path::os_str_into_bstr(rela_root.as_os_str())
                        .map(|rela_root| gix::path::to_unix_separators_on_windows(rela_root))
                })
                .transpose()?
                .map(|cow| cow.into_owned()),
        )
        .with_context(|| {
            format!(
                "failed to begin git status for repo {}",
                repo.path().display()
            )
        })?;

    let workdir = repo.workdir().unwrap();
    for status in statuses {
        let status = status.with_context(|| {
            format!(
                "failed to retrieve git status from repo {}",
                repo.path().display()
            )
        })?;
        let path = workdir.join(gix::path::from_bstr(status.location()));

        // It is OK to include Cargo.lock even if it is ignored.
        if path.ends_with("Cargo.lock")
            && matches!(
                &status,
                gix::status::Item::IndexWorktree(
                    gix::status::index_worktree::Item::DirectoryContents { entry, .. }
                ) if matches!(entry.status, gix::dir::entry::Status::Ignored(_))
            )
        {
            continue;
        }

        // We completely ignore submodules
        if matches!(
                status,
                gix::status::Item::TreeIndex(change) if change.entry_mode().is_commit())
        {
            continue;
        }
        dirty_files.push(path);
    }
    Ok(())
}

/// Make `pkg_root` relative to the `repo` workdir.
fn relative_package_root(repo: &gix::Repository, pkg_root: &Path) -> Option<PathBuf> {
    let workdir = repo.workdir().unwrap();
    let rela_root = pkg_root.strip_prefix(workdir).unwrap_or(Path::new(""));
    if rela_root.as_os_str().is_empty() {
        None
    } else {
        rela_root.to_owned().into()
    }
}
