use std::collections::{BTreeSet, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use super::{fingerprint, Context, Unit};
use util::paths;
use util::{internal, CargoResult};

fn dep_info_basedir(cx: &Context<'_, '_>) -> CargoResult<Option<String>> {
    cx.bcx
        .config
        .get_string("build.dep-info-basedir")
        .map(|option| option.map(|o| o.val))
}

fn render_filename(
    path: impl AsRef<Path>,
    basedir: Option<impl AsRef<Path>>,
) -> CargoResult<String> {
    let (path, basedir) = (path.as_ref(), basedir.as_ref());

    basedir
        .and_then(|base| path.strip_prefix(base).ok())
        .unwrap_or(path)
        .to_str()
        .ok_or_else(|| internal("path not utf-8"))
        .map(|f| f.replace(" ", "\\ "))
}

fn add_deps_for_unit<'a, 'b>(
    deps: &mut BTreeSet<PathBuf>,
    context: &mut Context<'a, 'b>,
    unit: &Unit<'a>,
    visited: &mut HashSet<Unit<'a>>,
    transitive: bool,
) -> CargoResult<()> {
    if !visited.insert(*unit) {
        return Ok(());
    }

    // units representing the execution of a build script don't actually
    // generate a dep info file, so we just keep on going below
    if !unit.mode.is_run_custom_build() {
        // Add dependencies from rustc dep-info output (stored in fingerprint directory)
        let dep_info_loc = fingerprint::dep_info_loc(context, unit);
        if let Some(paths) = fingerprint::parse_dep_info(unit.pkg, &dep_info_loc)? {
            for path in paths {
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
    if transitive || unit.mode.is_run_custom_build() {
        let key = (unit.pkg.package_id().clone(), unit.kind);
        if let Some(output) = context.build_state.outputs.lock().unwrap().get(&key) {
            for path in &output.rerun_if_changed {
                let path = if !path.is_absolute() {
                    unit.pkg.root().join(path)
                } else {
                    path.to_owned()
                };

                deps.insert(path);
            }
        }
    }

    // Recursively traverse all transitive dependencies
    if transitive {
        for dep_unit in context.dep_targets(unit).iter() {
            let source_id = dep_unit.pkg.package_id().source_id();
            if source_id.is_path() {
                add_deps_for_unit(deps, context, dep_unit, visited, transitive)?;
            }
        }
    }
    Ok(())
}

/// Returns a list of file dependencies for a given compilation `Unit`.
///
/// Inner `Result` type can fail if the actual dep-info generation failed; the
/// outer one is supposed to catch other, internal Cargo errors.
pub fn dep_files_for_unit<'a, 'b>(
    cx: &mut Context<'a, 'b>,
    unit: &Unit<'a>,
    transitive: bool,
) -> CargoResult<Result<BTreeSet<String>, ()>> {
    let basedir = dep_info_basedir(cx)?;

    let mut deps = BTreeSet::new();
    let mut visited = HashSet::new();
    Ok(match add_deps_for_unit(&mut deps, cx, unit, &mut visited, transitive) {
        Ok(_) => Ok(deps
            .iter()
            .map(|f| render_filename(f, basedir.as_ref()))
            .collect::<CargoResult<BTreeSet<_>>>()?),
        Err(_) => Err(()),
    })
}

pub fn output_depinfo<'a, 'b>(cx: &mut Context<'a, 'b>, unit: &Unit<'a>) -> CargoResult<()> {
    let basedir = dep_info_basedir(cx)?;

    let (success, deps) = match dep_files_for_unit(cx, unit, true)? {
        Ok(deps) => (true, deps.into_iter().collect()),
        _ => (false, vec![]),
    };

    for output in cx.outputs(unit)?.iter() {
        if let Some(ref link_dst) = output.hardlink {
            let output_path = link_dst.with_extension("d");
            if success {
                let target_fn = render_filename(link_dst, basedir.as_ref())?;

                // If nothing changed don't recreate the file which could alter
                // its mtime
                if let Ok(previous) = fingerprint::parse_rustc_dep_info(&output_path) {
                    if previous.len() == 1 && previous[0].0 == target_fn && previous[0].1 == deps {
                        continue;
                    }
                }

                // Otherwise write it all out
                let mut outfile = BufWriter::new(File::create(output_path)?);
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
