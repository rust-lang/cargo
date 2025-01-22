//! Helpers to gather the VCS information for `cargo package`.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as _;
use cargo_util::paths;
use cargo_util_schemas::manifest::TomlManifest;
use serde::Serialize;
use tracing::debug;

use crate::core::Package;
use crate::core::Workspace;
use crate::core::WorkspaceRootConfig;
use crate::sources::PathEntry;
use crate::CargoResult;
use crate::GlobalContext;

use super::PackageOpts;

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
    /// Indicate whether or not the Git worktree is dirty.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    dirty: bool,
}

/// Checks if the package source is in a *git* DVCS repository.
///
/// If *git*, and the source is *dirty* (e.g., has uncommitted changes),
/// and `--allow-dirty` has not been passed,
/// then `bail!` with an informative message.
/// Otherwise return the sha1 hash of the current *HEAD* commit,
/// or `None` if no repo is found.
#[tracing::instrument(skip_all)]
pub fn check_repo_state(
    p: &Package,
    src_files: &[PathEntry],
    ws: &Workspace<'_>,
    opts: &PackageOpts<'_>,
) -> CargoResult<Option<VcsInfo>> {
    let gctx = ws.gctx();
    let Ok(repo) = git2::Repository::discover(p.root()) else {
        gctx.shell().verbose(|shell| {
            shell.warn(format!("no (git) VCS found for `{}`", p.root().display()))
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
    let path = paths::strip_prefix_canonical(path, workdir).unwrap_or_else(|_| path.to_path_buf());
    let Ok(status) = repo.status_file(&path) else {
        gctx.shell().verbose(|shell| {
            shell.warn(format!(
                "no (git) Cargo.toml found at `{}` in workdir `{}`",
                path.display(),
                workdir.display()
            ))
        })?;
        // No checked-in `Cargo.toml` found. This package may be irrelevant.
        // Have to assume it is clean.
        return Ok(None);
    };

    if !(status & git2::Status::IGNORED).is_empty() {
        gctx.shell().verbose(|shell| {
            shell.warn(format!(
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
    let path_in_vcs = path
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .replace("\\", "/");
    let Some(git) = git(p, ws, src_files, &repo, &opts)? else {
        // If the git repo lacks essensial field like `sha1`, and since this field exists from the beginning,
        // then don't generate the corresponding file in order to maintain consistency with past behavior.
        return Ok(None);
    };

    return Ok(Some(VcsInfo { git, path_in_vcs }));
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
    repo: &git2::Repository,
) -> CargoResult<()> {
    if repo
        .config()
        .and_then(|c| c.get_bool("core.symlinks"))
        .unwrap_or(true)
    {
        return Ok(());
    }

    if src_files.iter().any(|f| f.maybe_plain_text_symlink()) {
        let mut shell = gctx.shell();
        shell.warn(format_args!(
            "found symbolic links that may be checked out as regular files for git repo at `{}`\n\
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
    pkg: &Package,
    ws: &Workspace<'_>,
    src_files: &[PathEntry],
    repo: &git2::Repository,
    opts: &PackageOpts<'_>,
) -> CargoResult<Option<GitVcsInfo>> {
    let gctx = ws.gctx();
    // This is a collection of any dirty or untracked files. This covers:
    // - new/modified/deleted/renamed/type change (index or worktree)
    // - untracked files (which are "new" worktree files)
    // - ignored (in case the user has an `include` directive that
    //   conflicts with .gitignore).
    let mut dirty_files = Vec::new();
    let pathspec = relative_pathspec(repo, pkg.root());
    collect_statuses(repo, &[pathspec.as_str()], &mut dirty_files)?;

    // Include each submodule so that the error message can provide
    // specifically *which* files in a submodule are modified.
    status_submodules(repo, &mut dirty_files)?;

    // Find the intersection of dirty in git, and the src_files that would
    // be packaged. This is a lazy n^2 check, but seems fine with
    // thousands of files.
    let cwd = gctx.cwd();
    let mut dirty_src_files: Vec<_> = src_files
        .iter()
        .filter(|src_file| dirty_files.iter().any(|path| src_file.starts_with(path)))
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
        // Must check whetherthe repo has no commit firstly, otherwise `revparse_single` would fail on bare commit repo.
        // Due to lacking the `sha1` field, it's better not record the `GitVcsInfo` for consistency.
        if repo.is_empty()? {
            return Ok(None);
        }
        let rev_obj = repo.revparse_single("HEAD")?;
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

/// Checks whether "included" source files outside package root have been modified.
///
/// This currently looks at
///
/// * `package.readme` and `package.license-file` pointing to paths outside package root
/// * symlinks targets reside outside package root
///
/// This is required because those paths may link to a file outside the
/// current package root, but still under the git workdir, affecting the
/// final packaged `.crate` file.
fn dirty_files_outside_pkg_root(
    ws: &Workspace<'_>,
    pkg: &Package,
    repo: &git2::Repository,
    src_files: &[PathEntry],
) -> CargoResult<HashSet<PathBuf>> {
    let pkg_root = pkg.root();
    let workdir = repo.workdir().unwrap();

    let meta = pkg.manifest().metadata();
    let metadata_paths: Vec<_> = [&meta.license_file, &meta.readme]
        .into_iter()
        .filter_map(|p| p.as_deref())
        .map(|path| paths::normalize_path(&pkg_root.join(path)))
        .collect();

    let mut dirty_files = HashSet::new();
    for rel_path in src_files
        .iter()
        .filter(|p| p.is_symlink_or_under_symlink())
        .map(|p| p.as_ref())
        .chain(metadata_paths.iter())
        // If inside package root. Don't bother checking git status.
        .filter(|p| paths::strip_prefix_canonical(p, pkg_root).is_err())
        // Handle files outside package root but under git workdir,
        .filter_map(|p| paths::strip_prefix_canonical(p, workdir).ok())
    {
        if repo.status_file(&rel_path)? != git2::Status::CURRENT {
            dirty_files.insert(workdir.join(rel_path));
        }
    }

    if let Some(dirty_ws_manifest) = dirty_workspace_manifest(ws, pkg, repo)? {
        dirty_files.insert(dirty_ws_manifest);
    }
    Ok(dirty_files)
}

fn dirty_workspace_manifest(
    ws: &Workspace<'_>,
    pkg: &Package,
    repo: &git2::Repository,
) -> CargoResult<Option<PathBuf>> {
    let workdir = repo.workdir().unwrap();
    let ws_manifest_path = ws.root_manifest();
    if pkg.manifest_path() == ws_manifest_path {
        // The workspace manifest is also the primary package manifest.
        // Normal file statuc check should have covered it.
        return Ok(None);
    }
    if paths::strip_prefix_canonical(ws_manifest_path, pkg.root()).is_ok() {
        // Inside package root. Don't bother checking git status.
        return Ok(None);
    }
    let Ok(rel_path) = paths::strip_prefix_canonical(ws_manifest_path, workdir) else {
        // Completely outside this git workdir.
        return Ok(None);
    };

    // Outside package root but under git workdir.
    if repo.status_file(&rel_path)? == git2::Status::CURRENT {
        return Ok(None);
    }

    let from_index = ws_manifest_and_root_config_from_index(ws, repo, &rel_path);
    // If there is no workable workspace manifest in Git index,
    // create a default inheritable fields.
    // With it, we can detect any member manifest has inherited fields,
    // and then the workspace manifest should be considered dirty.
    let inheritable = if let Some(fields) = from_index
        .as_ref()
        .map(|(_, root_config)| root_config.inheritable())
    {
        fields
    } else {
        &Default::default()
    };

    let empty = Vec::new();
    let cargo_features = crate::core::Features::new(
        from_index
            .as_ref()
            .and_then(|(manifest, _)| manifest.cargo_features.as_ref())
            .unwrap_or(&empty),
        ws.gctx(),
        &mut Default::default(),
        pkg.package_id().source_id().is_path(),
    )
    .unwrap_or_default();

    let dirty_path = || Ok(Some(workdir.join(&rel_path)));
    let dirty = |msg| {
        debug!(
            "{msg} for `{}` of repo at `{}`",
            rel_path.display(),
            workdir.display(),
        );
        dirty_path()
    };

    let Ok(normalized_toml) = crate::util::toml::normalize_toml(
        pkg.manifest().original_toml(),
        &cargo_features,
        &|| Ok(inheritable),
        pkg.manifest_path(),
        ws.gctx(),
        &mut Default::default(),
        &mut Default::default(),
    ) else {
        return dirty("failed to normalize pkg manifest from index");
    };

    let Ok(from_index) = toml::to_string_pretty(&normalized_toml) else {
        return dirty("failed to serialize pkg manifest from index");
    };

    let Ok(from_working_dir) = toml::to_string_pretty(pkg.manifest().normalized_toml()) else {
        return dirty("failed to serialize pkg manifest from working directory");
    };

    if from_index != from_working_dir {
        tracing::trace!("--- from index ---\n{from_index}");
        tracing::trace!("--- from working dir ---\n{from_working_dir}");
        return dirty("normalized manifests from index and in working directory mismatched");
    }

    Ok(None)
}

/// Gets workspace manifest and workspace root config from Git index.
///
/// This returns an `Option` because workspace manifest might be broken or not
/// exist at all.
fn ws_manifest_and_root_config_from_index(
    ws: &Workspace<'_>,
    repo: &git2::Repository,
    ws_manifest_rel_path: &Path,
) -> Option<(TomlManifest, WorkspaceRootConfig)> {
    let workdir = repo.workdir().unwrap();
    let dirty = |msg| {
        debug!(
            "{msg} for `{}` of repo at `{}`",
            ws_manifest_rel_path.display(),
            workdir.display(),
        );
        None
    };
    let Ok(index) = repo.index() else {
        debug!("no index for repo at `{}`", workdir.display());
        return None;
    };
    let Some(entry) = index.get_path(ws_manifest_rel_path, 0) else {
        return dirty("workspace manifest not found");
    };
    let Ok(blob) = repo.find_blob(entry.id) else {
        return dirty("failed to find manifest blob");
    };
    let Ok(contents) = String::from_utf8(blob.content().to_vec()) else {
        return dirty("failed parse as UTF-8 encoding");
    };
    let Ok(document) = crate::util::toml::parse_document(&contents) else {
        return dirty("failed to parse file");
    };
    let Ok(ws_manifest_from_index) = crate::util::toml::deserialize_toml(&document) else {
        return dirty("failed to deserialize doc");
    };
    let Some(toml_workspace) = ws_manifest_from_index.workspace.as_ref() else {
        return dirty("not a workspace manifest");
    };

    let ws_root_config =
        crate::util::toml::to_workspace_root_config(toml_workspace, ws.root_manifest());
    Some((ws_manifest_from_index, ws_root_config))
}

/// Helper to collect dirty statuses for a single repo.
fn collect_statuses(
    repo: &git2::Repository,
    pathspecs: &[&str],
    dirty_files: &mut Vec<PathBuf>,
) -> CargoResult<()> {
    let mut status_opts = git2::StatusOptions::new();
    // Exclude submodules, as they are being handled manually by recursing
    // into each one so that details about specific files can be
    // retrieved.
    pathspecs
        .iter()
        .fold(&mut status_opts, git2::StatusOptions::pathspec)
        .exclude_submodules(true)
        .include_ignored(true)
        .include_untracked(true);
    let repo_statuses = repo.statuses(Some(&mut status_opts)).with_context(|| {
        format!(
            "failed to retrieve git status from repo {}",
            repo.path().display()
        )
    })?;
    let workdir = repo.workdir().unwrap();
    let this_dirty = repo_statuses.iter().filter_map(|entry| {
        let path = entry.path().expect("valid utf-8 path");
        if path.ends_with("Cargo.lock") && entry.status() == git2::Status::IGNORED {
            // It is OK to include Cargo.lock even if it is ignored.
            return None;
        }
        // Use an absolute path, so that comparing paths is easier
        // (particularly with submodules).
        Some(workdir.join(path))
    });
    dirty_files.extend(this_dirty);
    Ok(())
}

/// Helper to collect dirty statuses while recursing into submodules.
fn status_submodules(repo: &git2::Repository, dirty_files: &mut Vec<PathBuf>) -> CargoResult<()> {
    for submodule in repo.submodules()? {
        // Ignore submodules that don't open, they are probably not initialized.
        // If its files are required, then the verification step should fail.
        if let Ok(sub_repo) = submodule.open() {
            status_submodules(&sub_repo, dirty_files)?;
            collect_statuses(&sub_repo, &[], dirty_files)?;
        }
    }
    Ok(())
}

/// Use pathspec so git only matches a certain path prefix
fn relative_pathspec(repo: &git2::Repository, pkg_root: &Path) -> String {
    let workdir = repo.workdir().unwrap();
    let relpath = pkg_root.strip_prefix(workdir).unwrap_or(Path::new(""));
    // to unix separators
    relpath.to_str().unwrap().replace('\\', "/")
}
