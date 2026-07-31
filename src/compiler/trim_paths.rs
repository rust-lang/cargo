//! Path prefix remapping for [RFC 3127] `trim-paths`.
//!
//! [RFC 3127]: https://rust-lang.github.io/rfcs/3127-trim-paths.html

use std::ffi::OsString;
use std::path::Path;

use cargo_util::ProcessBuilder;
use cargo_util_schemas::manifest::TomlTrimPaths;
use cargo_util_schemas::manifest::TomlTrimPathsValue;

use super::BuildRunner;
use super::Unit;
use crate::util::errors::CargoResult;
use crate::util::hex;

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
        package_remap(build_runner, unit),
        build_dir_remap(build_runner),
        sysroot_remap(build_runner, unit),
    ]
}

/// Path prefix remap rules for sysroot.
///
/// This remap logic aligns with rustc:
/// <https://github.com/rust-lang/rust/blob/c2ef3516/src/bootstrap/src/lib.rs#L1113-L1116>
fn sysroot_remap(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> OsString {
    let mut remap = OsString::new();
    remap.push({
        // See also `detect_sysroot_src_path()`.
        let mut sysroot = build_runner.bcx.target_data.info(unit.kind).sysroot.clone();
        sysroot.push("lib");
        sysroot.push("rustlib");
        sysroot.push("src");
        sysroot.push("rust");
        sysroot
    });
    remap.push("=");
    remap.push("/rustc/");
    if let Some(commit_hash) = build_runner.bcx.rustc().commit_hash.as_ref() {
        remap.push(commit_hash);
    } else {
        remap.push(build_runner.bcx.rustc().version.to_string());
    }
    remap
}

/// Path prefix remap rules for dependencies.
fn package_remap(build_runner: &BuildRunner<'_, '_>, unit: &Unit) -> OsString {
    let pkg_root = unit.pkg.root();
    let ws_root = build_runner.bcx.ws.root();
    let mut remap = OsString::new();
    let source_id = unit.pkg.package_id().source_id();

    if source_id.is_git() {
        if let Some((from, rev)) = git_checkout(build_runner, pkg_root) {
            const GIT_OID_LEN: usize = 7; // This matches MIN_ABBREV_LEN in git source
            remap.push(from);
            remap.push("=/cargo/git/");
            remap.push(hex::short_hash(source_id.canonical_url()));
            remap.push("/");
            remap.push(&rev[..rev.len().min(GIT_OID_LEN)]);
            return remap;
        }
    } else if source_id.is_registry() {
        let registry_src = build_runner.bcx.gctx.registry_source_path();
        let registry_src = registry_src.as_path_unlocked();
        let from = pkg_root.parent().unwrap();
        if from.starts_with(registry_src) {
            remap.push(from);
            remap.push("=/cargo/registry/");
            remap.push(hex::short_hash(&source_id));
            return remap;
        }
    }

    // Handle path local dependencies and abnormal reg/git deps source location.
    if pkg_root.strip_prefix(ws_root).is_ok() {
        remap.push(ws_root);
        remap.push("=."); // remap to relative rustc work dir explicitly
    } else {
        remap.push(pkg_root);
        remap.push("=/cargo/path/");
        remap.push(unit.pkg.name());
        remap.push("-");
        remap.push(unit.pkg.version().to_string());
    }
    remap
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
fn build_dir_remap(build_runner: &BuildRunner<'_, '_>) -> OsString {
    let build_dir = build_runner.bcx.ws.build_dir();
    let mut remap = OsString::new();
    remap.push(build_dir.as_path_unlocked());
    remap.push("=/cargo/build-dir");
    remap
}
