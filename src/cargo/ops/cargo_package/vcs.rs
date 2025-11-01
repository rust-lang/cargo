//! Helpers to gather the VCS information for `cargo package`.

use crate::core::{Package, Workspace};
use crate::ops::PackageOpts;
use crate::sources::PathEntry;
use crate::{CargoResult, GlobalContext};
use annotate_snippets::Level;
use anyhow::Context;
use cargo_util::paths;
use gix::bstr::ByteSlice;
use gix::dir::walk::EmissionMode;
use gix::dirwalk::Options;
use gix::index::entry::Mode;
use gix::status::tree_index::TrackRenames;
use gix::worktree::stack::state::ignore::Source;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Represents the VCS information when packaging.
#[derive(Serialize)]
pub struct VcsInfo {
    git: GitVcsInfo,
    /// Path to the package within repo (empty string if root).
    path_in_vcs: String,
}

/// Represents the Git VCS information when packaging.
#[derive(Serialize)]
pub struct GitVcsInfo {
    sha1: String,
    /// Indicate whether the Git worktree is dirty.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    dirty: bool,
}

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
    let Ok(mut repo) = gix::discover(p.root()) else {
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
    let Some(git) = git(ws, p, src_files, &mut repo, &opts)? else {
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
        let msg = format!(
            "found symbolic links that may be checked out as regular files for git repo at `{}/`",
            repo.workdir().unwrap().display()
        );
        let mut notes = vec![
            Level::NOTE.message(
                "this might cause the `.crate` file to include incorrect or incomplete files",
            ),
            Level::HELP.message("to avoid this, set the Git config `core.symlinks` to `true`"),
        ];
        if cfg!(windows) {
            notes.push(
                Level::HELP.message("on Windows, enable the Developer Mode to support symlinks"),
            );
        };
        gctx.shell().print_report(
            &[Level::WARNING
                .secondary_title(msg)
                .elements(notes.into_iter())],
            false,
        )?;
    }

    Ok(())
}

/// The real git status check starts from here.
fn git(
    ws: &Workspace<'_>,
    pkg: &Package,
    src_files: &[PathEntry],
    repo: &mut gix::Repository,
    opts: &PackageOpts<'_>,
) -> CargoResult<Option<GitVcsInfo>> {
    {
        let mut config = repo.config_snapshot_mut();
        // This currently is only a very minor speedup for the biggest repositories,
        // but might trigger creating many threads.
        config.set_value(&gix::config::tree::Index::THREADS, "false")?;
    }
    // This is a collection of any dirty or untracked files. This covers:
    // - new/modified/deleted/renamed/type change (index or worktree)
    // - untracked files (which are "new" worktree files)
    // - ignored (in case the user has an `include` directive that
    //   conflicts with .gitignore).
    let mut dirty_files = Vec::new();
    let workdir = repo.workdir().unwrap();
    collect_statuses(
        repo,
        workdir,
        relative_package_root(repo, pkg.root()).as_deref(),
        &mut dirty_files,
    )?;

    // Include each submodule so that the error message can provide
    // specifically *which* files in a submodule are modified.
    status_submodules(repo, &mut dirty_files)?;

    // Find the intersection of dirty in git, and the src_files that would
    // be packaged. This is a lazy n^2 check, but seems fine with
    // thousands of files.
    let cwd = ws.gctx().cwd();
    let mut dirty_src_files: Vec<_> = src_files
        .iter()
        .filter(|src_file| {
            if let Some(canon_src_file) = src_file.is_symlink_or_under_symlink().then(|| {
                gix::path::realpath_opts(
                    &src_file,
                    ws.gctx().cwd(),
                    gix::path::realpath::MAX_SYMLINKS,
                )
                .unwrap_or_else(|_| src_file.to_path_buf())
            }) {
                dirty_files
                    .iter()
                    .any(|path| canon_src_file.starts_with(path))
            } else {
                dirty_files.iter().any(|path| src_file.starts_with(path))
            }
        })
        .map(|p| p.as_ref())
        .chain(dirty_files_outside_pkg_root(ws, pkg, repo, src_files)?.iter())
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
        let maybe_head_id = repo.head()?.try_peel_to_id()?;
        Ok(maybe_head_id.map(|id| GitVcsInfo {
            sha1: id.to_string(),
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
/// Writes dirty files outside `relative_package_root` into `dirty_files_outside_package_root`,
/// and all *everything else* into `dirty_files`.
#[must_use]
fn collect_statuses(
    repo: &gix::Repository,
    workdir: &Path,
    relative_package_root: Option<&Path>,
    dirty_files: &mut Vec<PathBuf>,
) -> CargoResult<()> {
    let statuses = repo
        .status(gix::progress::Discard)?
        .dirwalk_options(configure_dirwalk)
        .tree_index_track_renames(TrackRenames::Disabled)
        .index_worktree_submodules(None)
        .into_iter(
            relative_package_root.map(|rela_pkg_root| {
                gix::path::into_bstr(rela_pkg_root).into_owned()
            }), /* pathspec patterns */
        )
        .with_context(|| {
            format!(
                "failed to begin git status for repo {}",
                repo.path().display()
            )
        })?;

    for status in statuses {
        let status = status.with_context(|| {
            format!(
                "failed to retrieve git status from repo {}",
                repo.path().display()
            )
        })?;

        let rel_path = gix::path::from_bstr(status.location());
        let path = workdir.join(&rel_path);
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

        dirty_files.push(path);
    }
    Ok(())
}

/// Helper to collect dirty statuses while recursing into submodules.
fn status_submodules(repo: &gix::Repository, dirty_files: &mut Vec<PathBuf>) -> CargoResult<()> {
    let Some(submodules) = repo.submodules()? else {
        return Ok(());
    };
    for submodule in submodules {
        // Ignore submodules that don't open, they are probably not initialized.
        // If its files are required, then the verification step should fail.
        if let Some(sub_repo) = submodule.open()? {
            let Some(workdir) = sub_repo.workdir() else {
                continue;
            };
            status_submodules(&sub_repo, dirty_files)?;
            collect_statuses(&sub_repo, workdir, None, dirty_files)?;
        }
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

/// Checks whether "included" source files outside package root have been modified.
///
/// This currently looks at
///
/// * `package.readme` and `package.license-file` pointing to paths outside package root
/// * symlinks targets residing outside package root
/// * Any change in the root workspace manifest, regardless of what has changed.
///
/// This is required because those paths may link to a file outside the
/// current package root, but still under the git workdir, affecting the
/// final packaged `.crate` file.
fn dirty_files_outside_pkg_root(
    ws: &Workspace<'_>,
    pkg: &Package,
    repo: &gix::Repository,
    src_files: &[PathEntry],
) -> CargoResult<Vec<PathBuf>> {
    let pkg_root = pkg.root();
    let workdir = repo.workdir().unwrap();

    let meta = pkg.manifest().metadata();
    let metadata_paths: Vec<_> = [&meta.license_file, &meta.readme]
        .into_iter()
        .filter_map(|p| p.as_deref())
        .map(|path| paths::normalize_path(&pkg_root.join(path)))
        .collect();

    let linked_files_outside_package_root: Vec<_> = src_files
        .iter()
        .filter(|p| p.is_symlink_or_under_symlink())
        .map(|p| p.as_ref().as_path())
        .chain(metadata_paths.iter().map(AsRef::as_ref))
        .chain([ws.root_manifest()])
        // If inside package root. Don't bother checking git status.
        .filter(|p| paths::strip_prefix_canonical(p, pkg_root).is_err())
        // Handle files outside package root but under git workdir,
        .filter_map(|p| paths::strip_prefix_canonical(p, workdir).ok())
        .collect();

    if linked_files_outside_package_root.is_empty() {
        return Ok(Vec::new());
    }

    let statuses = repo
        .status(gix::progress::Discard)?
        .dirwalk_options(configure_dirwalk)
        // Limit the amount of threads for used for the worktree status, as the pathspec will
        // prevent most paths from being visited anyway there is not much work.
        .index_worktree_options_mut(|opts| opts.thread_limit = Some(1))
        .tree_index_track_renames(TrackRenames::Disabled)
        .index_worktree_submodules(None)
        .into_iter(
            linked_files_outside_package_root
                .into_iter()
                .map(|p| gix::path::into_bstr(p).into_owned()),
        )
        .with_context(|| {
            format!(
                "failed to begin git status for outfor repo {}",
                repo.path().display()
            )
        })?;

    let mut dirty_files = Vec::new();
    for status in statuses {
        let status = status.with_context(|| {
            format!(
                "failed to retrieve git status from repo {}",
                repo.path().display()
            )
        })?;

        let rel_path = gix::path::from_bstr(status.location());
        let path = workdir.join(&rel_path);
        dirty_files.push(path);
    }
    Ok(dirty_files)
}

fn configure_dirwalk(opts: Options) -> Options {
    opts.emit_untracked(gix::dir::walk::EmissionMode::Matching)
        // Also pick up ignored files or whole directories
        // to specifically catch overzealously ignored source files.
        // Later we will match these dirs by prefix, which is why collapsing
        // them is desirable here.
        .emit_ignored(Some(EmissionMode::CollapseDirectory))
        .emit_tracked(false)
        .recurse_repositories(false)
        .symlinks_to_directories_are_ignored_like_directories(true)
        .emit_empty_directories(false)
}
