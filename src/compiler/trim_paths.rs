//! Path prefix remapping for [RFC 3127] `trim-paths`.
//!
//! [RFC 3127]: https://rust-lang.github.io/rfcs/3127-trim-paths.html

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use cargo_util::ProcessBuilder;
use cargo_util_schemas::manifest::TomlTrimPaths;
use cargo_util_schemas::manifest::TomlTrimPathsValue;
use serde::Serialize;
use tracing::debug;

use super::BuildRunner;
use super::Unit;
use crate::util::data_structures::HashSet;
use crate::util::errors::CargoResult;
use crate::util::hex;

/// The current version of the unremap file.
const CURRENT_UNREMAP_VERSION: u8 = 1;

/// Filename suffix of the unremap file.
pub(crate) const UNREMAP_SUFFIX: &str = ".trim-paths.jsonl";

/// Like [`trim_paths_args`] but for rustdoc invocations.
pub(crate) fn trim_paths_args_rustdoc(
    cmd: &mut ProcessBuilder,
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
    trim_paths: &TomlTrimPaths,
) -> CargoResult<()> {
    match trim_paths {
        // rustdoc supports diagnostics trimming only.
        TomlTrimPaths::Values(values) if !values.contains(&TomlTrimPathsValue::Diagnostics) => {
            return Ok(());
        }
        _ => {}
    }

    for pair in trim_paths_remap(build_runner, unit) {
        let mut arg = OsString::from("--remap-path-prefix=");
        arg.push(pair);
        cmd.arg(arg);
    }

    Ok(())
}

/// Generates the `--remap-path-scope` and `--remap-path-prefix` for [RFC 3127].
/// See also unstable feature [`-Ztrim-paths`].
///
/// [RFC 3127]: https://rust-lang.github.io/rfcs/3127-trim-paths.html
/// [`-Ztrim-paths`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#profile-trim-paths-option
pub(crate) fn trim_paths_args(
    cmd: &mut ProcessBuilder,
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
    trim_paths: &TomlTrimPaths,
) -> CargoResult<()> {
    if trim_paths.is_none() {
        return Ok(());
    }

    // feature gate was checked during manifest/config parsing.
    cmd.arg(format!("--remap-path-scope={trim_paths}"));

    for pair in trim_paths_remap(build_runner, unit) {
        let mut arg = OsString::from("--remap-path-prefix=");
        arg.push(pair);
        cmd.arg(arg);
    }

    Ok(())
}

/// Computes the `<from>=<to>` path remap pairs for [RFC 3127] trim-paths.
///
/// Order of `--remap-path-prefix` flags is important for `-Zbuild-std`.
/// We want to show `/rustc/<hash>/library/std` instead of `std-0.0.0`.
///
/// | Category            | From                                         | To                                 |
/// |---------------------|----------------------------------------------|------------------------------------|
/// | Sysroot             | `<sysroot>/lib/rustlib/src/rust`             | `/rustc/<commit-hash>`             |
/// | Registry dep        | `$CARGO_HOME/registry/src/<registry-dir>`        | `/cargo/registry/<registry-id>`    |
/// | Git dep             | `$CARGO_HOME/git/checkouts/<repo-dir>/<rev-dir>` | `/cargo/git/<git-source-id>/<rev>` |
/// | Workspace           | `<workspace-root>`                           | `.` (workspace-relative)           |
/// | Path dep outside ws | `<pkg-root>`                                 | `/cargo/path/<name>-<version>`     |
/// | Vendored            | `<pkg-root>` (by file location)              | workspace or path rules above      |
/// | Build directory     | `<build-dir>`                                | `/cargo/build-dir`                 |
///
/// [RFC 3127]: https://rust-lang.github.io/rfcs/3127-trim-paths.html
pub(crate) fn trim_paths_remap(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> [OsString; 3] {
    [
        join_remap(package_remap(build_runner, unit)),
        join_remap(build_dir_remap(build_runner)),
        join_remap(sysroot_remap(build_runner)),
    ]
}

fn join_remap((from, to): (PathBuf, String)) -> OsString {
    let mut remap = OsString::with_capacity(from.as_os_str().len() + 1 + to.len());
    remap.push(from);
    remap.push("=");
    remap.push(to);
    remap
}

/// Path prefix remap rules for sysroot.
///
/// This remap logic aligns with rustc:
/// <https://github.com/rust-lang/rust/blob/c2ef3516/src/bootstrap/src/lib.rs#L1113-L1116>
fn sysroot_remap(build_runner: &BuildRunner<'_, '_>) -> (PathBuf, String) {
    // See also `detect_sysroot_src_path()`.
    let sysroot = build_runner
        .bcx
        .get_sysroot()
        .join("lib")
        .join("rustlib")
        .join("src")
        .join("rust");

    let rustc = build_runner.bcx.rustc();
    let to = match rustc.commit_hash.as_ref() {
        Some(commit_hash) => format!("/rustc/{commit_hash}"),
        None => format!("/rustc/{}", rustc.version),
    };
    (sysroot, to)
}

/// Path prefix remap rules for dependencies.
fn package_remap(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> (PathBuf, String) {
    let pkg_root = unit.pkg.root();
    let ws_root = build_runner.bcx.ws.root();
    let source_id = unit.pkg.package_id().source_id();

    if source_id.is_git() {
        if let Some((from, rev)) = git_checkout(build_runner, pkg_root) {
            const GIT_OID_LEN: usize = 7; // This matches MIN_ABBREV_LEN in git source
            let repo = hex::short_hash(source_id.canonical_url());
            let rev = &rev[..rev.len().min(GIT_OID_LEN)];
            return (from.to_path_buf(), format!("/cargo/git/{repo}/{rev}"));
        }
    } else if source_id.is_registry() {
        let registry_src = build_runner.bcx.gctx.registry_source_path();
        let registry_src = registry_src.as_path_unlocked();
        let from = pkg_root.parent().unwrap();
        if from.starts_with(registry_src) {
            let registry = hex::short_hash(&source_id);
            return (from.to_path_buf(), format!("/cargo/registry/{registry}"));
        }
    }

    // Handle path local dependencies and abnormal reg/git deps source location.
    if pkg_root.strip_prefix(ws_root).is_ok() {
        // remap to relative rustc work dir explicitly
        (ws_root.to_path_buf(), ".".to_owned())
    } else {
        let from = pkg_root.to_path_buf();
        let to = format!("/cargo/path/{}-{}", unit.pkg.name(), unit.pkg.version());
        (from, to)
    }
}

/// Finds the checkout root and revision directory name of a git dependency.
///
/// This is built under this layout: `$CARGO_HOME/git/checkouts/<repo>-<hash>[-shallow]/<rev>`.
///
/// `None` when the package does not live under the global git checkouts directory,
/// for example a vendored git dependency.
fn git_checkout<'a>(
    build_runner: &BuildRunner<'_, '_>,
    pkg_root: &'a Path,
) -> Option<(&'a Path, &'a str)> {
    let checkouts = build_runner.bcx.gctx.git_checkouts_path();
    let checkouts = checkouts.as_path_unlocked();
    let rel = pkg_root.strip_prefix(checkouts).ok()?;
    let mut components = rel.components();
    let (_repo, rev) = (components.next()?, components.next()?);
    let rev = rev.as_os_str().to_str()?;
    let checkout_root = pkg_root.ancestors().nth(components.count())?;
    Some((checkout_root, rev))
}

/// Remap all paths pointing to `build.build-dir`,
/// i.e., `[BUILD_DIR]/debug/deps/foo-[HASH].dwo` would be remapped to
/// `/cargo/build-dir/debug/deps/foo-[HASH].dwo`
/// (note the `/cargo/build-dir` prefix).
///
/// This covers scenarios like:
///
/// * Build script generated code. For example, a build script may call `file!`
///   macros, and the associated crate uses [`include!`] to include the expanded
///   [`file!`] macro in-place via the `OUT_DIR` environment.
/// * On Linux, `DW_AT_GNU_dwo_name` that contains paths to split debuginfo
///   files (dwp and dwo).
fn build_dir_remap(build_runner: &BuildRunner<'_, '_>) -> (PathBuf, String) {
    let from = build_runner.bcx.ws.build_dir().into_path_unlocked();
    let to = "/cargo/build-dir".to_owned();
    (from, to)
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct UnremapVersion {
    v: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct UnremapMetadata<'a> {
    rust_version: &'a str,
    workspace_root: &'a Path,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct Remap<'a> {
    from: &'a str,
    to: &'a Path,
}

/// Whether an unremap file is worth emitting beside its artifacts.
pub(crate) fn should_emit_unremap_file(unit: &Unit) -> bool {
    // The unremap file is a debug companion like a dSYM or PDB.
    // Without debuginfo there is nothing worth unmapping.
    if !unit.profile.debuginfo.is_turned_on() {
        return false;
    }

    match unit.profile.trim_paths.as_ref() {
        None => false,
        Some(TomlTrimPaths::All) => true,
        Some(TomlTrimPaths::Values(values)) => values.contains(&TomlTrimPathsValue::Object),
    }
}

/// Writes the unremap file for a unit's final artifacts.
pub(crate) fn write_unremap_file(
    mut out: impl Write,
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
) -> CargoResult<()> {
    let mut remaps = BTreeMap::new();

    let mut insert = |(from, to): (PathBuf, String)| match remaps.entry(to) {
        Entry::Vacant(entry) => {
            entry.insert(from);
        }
        Entry::Occupied(entry) if *entry.get() != from => {
            debug!(
                "conflicting unremap records for `{}`: `{}` and `{}`",
                entry.key(),
                entry.get().display(),
                from.display(),
            );
        }
        Entry::Occupied(_) => {}
    };

    insert(sysroot_remap(build_runner));
    insert(build_dir_remap(build_runner));

    let mut seen = HashSet::default();
    let mut stack = vec![unit.clone()];
    while let Some(unit) = stack.pop() {
        if !seen.insert(unit.clone()) {
            continue;
        }
        for dep in build_runner.unit_deps(&unit) {
            stack.push(dep.unit.clone());
        }
        let (from, to) = package_remap(build_runner, &unit);

        // Skipping workspace remap because debugger substitutions are global in a session.
        // If two unremap files with different workspace roots are loaded.
        // substitutions of `.` would override each others.
        if to != "." {
            insert((from, to));
        }
    }

    serde_json::to_writer(
        &mut out,
        &UnremapVersion {
            v: CURRENT_UNREMAP_VERSION,
        },
    )?;
    out.write_all(b"\n")?;
    let rust_version = build_runner.bcx.rustc().version.to_string();
    let metadata = UnremapMetadata {
        rust_version: &rust_version,
        workspace_root: build_runner.bcx.ws.root(),
    };
    serde_json::to_writer(&mut out, &metadata)?;
    out.write_all(b"\n")?;
    for (from, to) in &remaps {
        serde_json::to_writer(&mut out, &Remap { from, to })?;
        out.write_all(b"\n")?;
    }

    Ok(())
}

/// Appends the unremap file suffix to an artifact path.
pub(crate) fn append_unremap_suffix(link: &PathBuf) -> PathBuf {
    let mut link_buf = link.clone().into_os_string();
    link_buf.push(UNREMAP_SUFFIX);
    PathBuf::from(link_buf)
}
