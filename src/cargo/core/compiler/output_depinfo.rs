//! dep-info files for external build system integration.
//! See [`output_depinfo`] for more.

use cargo_util::paths::normalize_path;
use std::collections::{BTreeSet, HashSet};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use super::{fingerprint, Context, FileFlavor, Unit};
use crate::util::{internal, CargoResult};
use cargo_util::paths;
use tracing::debug;

/// Bacially just normalizes a given path and converts it to a string.
fn render_filename<P: AsRef<Path>>(path: P, basedir: Option<&str>) -> CargoResult<String> {
    fn wrap_path(path: &Path) -> CargoResult<String> {
        path.to_str()
            .ok_or_else(|| internal(format!("path `{:?}` not utf-8", path)))
            .map(|f| f.replace(" ", "\\ "))
    }

    let path = path.as_ref();
    if let Some(basedir) = basedir {
        let norm_path = normalize_path(path);
        let norm_basedir = normalize_path(basedir.as_ref());
        match norm_path.strip_prefix(norm_basedir) {
            Ok(relpath) => wrap_path(relpath),
            _ => wrap_path(path),
        }
    } else {
        wrap_path(path)
    }
}

/// Collects all dependencies of the `unit` for the output dep info file.
///
/// Dependencies will be stored in `deps`, including:
///
/// * dependencies from [fingerprint dep-info]
/// * paths from `rerun-if-changed` build script instruction
/// * ...and traverse transitive dependencies recursively
///
/// [fingerprint dep-info]: super::fingerprint#fingerprint-dep-info-files
fn add_deps_for_unit(
    deps: &mut BTreeSet<PathBuf>,
    cx: &mut Context<'_, '_>,
    unit: &Unit,
    visited: &mut HashSet<Unit>,
) -> CargoResult<()> {
    if !visited.insert(unit.clone()) {
        return Ok(());
    }

    // units representing the execution of a build script don't actually
    // generate a dep info file, so we just keep on going below
    if !unit.mode.is_run_custom_build() {
        // Add dependencies from rustc dep-info output (stored in fingerprint directory)
        let dep_info_loc = fingerprint::dep_info_loc(cx, unit);
        if let Some(paths) =
            fingerprint::parse_dep_info(unit.pkg.root(), cx.files().host_root(), &dep_info_loc)?
        {
            for path in paths.files {
                deps.insert(path);
            }
        } else {
            debug!(
                "can't find dep_info for {:?} {}",
                unit.pkg.package_id(),
                unit.target
            );
            return Err(internal("dep_info missing"));
        }
    }

    // Add rerun-if-changed dependencies
    if let Some(metadata) = cx.find_build_script_metadata(unit) {
        if let Some(output) = cx.build_script_outputs.lock().unwrap().get(metadata) {
            for path in &output.rerun_if_changed {
                // The paths we have saved from the unit are of arbitrary relativeness and may be
                // relative to the crate root of the dependency.
                let path = unit.pkg.root().join(path);
                deps.insert(path);
            }
        }
    }

    // Recursively traverse all transitive dependencies
    let unit_deps = Vec::from(cx.unit_deps(unit)); // Create vec due to mutable borrow.
    for dep in unit_deps {
        if dep.unit.is_local() {
            add_deps_for_unit(deps, cx, &dep.unit, visited)?;
        }
    }
    Ok(())
}

/// Save a `.d` dep-info file for the given unit. This is the third kind of
/// dep-info mentioned in [`fingerprint`] module.
///
/// Argument `unit` is expected to be the root unit, which will be uplifted.
///
/// Cargo emits its own dep-info files in the output directory. This is
/// only done for every "uplifted" artifact. These are intended to be used
/// with external build systems so that they can detect if Cargo needs to be
/// re-executed.
///
/// It includes all the entries from the `rustc` dep-info file, and extends it
/// with any `rerun-if-changed` entries from build scripts. It also includes
/// sources from any path dependencies. Registry dependencies are not included
/// under the assumption that changes to them can be detected via changes to
/// `Cargo.lock`.
///
/// [`fingerprint`]: super::fingerprint#dep-info-files
pub fn output_depinfo(cx: &mut Context<'_, '_>, unit: &Unit) -> CargoResult<()> {
    let bcx = cx.bcx;
    let mut deps = BTreeSet::new();
    let mut visited = HashSet::new();
    let success = add_deps_for_unit(&mut deps, cx, unit, &mut visited).is_ok();
    let basedir_string;
    let basedir = match bcx.config.build_config()?.dep_info_basedir.clone() {
        Some(value) => {
            basedir_string = value
                .resolve_path(bcx.config)
                .as_os_str()
                .to_str()
                .ok_or_else(|| anyhow::format_err!("build.dep-info-basedir path not utf-8"))?
                .to_string();
            Some(basedir_string.as_str())
        }
        None => None,
    };
    let deps = deps
        .iter()
        .map(|f| render_filename(f, basedir))
        .collect::<CargoResult<Vec<_>>>()?;

    for output in cx
        .outputs(unit)?
        .iter()
        .filter(|o| !matches!(o.flavor, FileFlavor::DebugInfo | FileFlavor::Auxiliary))
    {
        if let Some(ref link_dst) = output.hardlink {
            let output_path = link_dst.with_extension("d");
            if success {
                let target_fn = render_filename(link_dst, basedir)?;

                // If nothing changed don't recreate the file which could alter
                // its mtime
                if let Ok(previous) = fingerprint::parse_rustc_dep_info(&output_path) {
                    if previous.files.iter().eq(deps.iter().map(Path::new)) {
                        continue;
                    }
                }

                // Otherwise write it all out
                let mut outfile = BufWriter::new(paths::create(output_path)?);
                write!(outfile, "{}:", target_fn)?;
                for dep in &deps {
                    write!(outfile, " {}", dep)?;
                }
                writeln!(outfile)?;

            // dep-info generation failed, so delete output file. This will
            // usually cause the build system to always rerun the build
            // rule, which is correct if inefficient.
            } else if output_path.exists() {
                paths::remove_file(output_path)?;
            }
        }
    }
    Ok(())
}
