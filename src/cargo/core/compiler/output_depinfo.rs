use std::collections::{BTreeSet, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use log::debug;

use super::{fingerprint, Context, FileFlavor, Unit};
use crate::util::paths;
use crate::util::{internal, CargoResult};

fn render_filename<P: AsRef<Path>>(path: P, basedir: Option<&str>) -> CargoResult<String> {
    let path = path.as_ref();
    let relpath = match basedir {
        None => path,
        Some(base) => match path.strip_prefix(base) {
            Ok(relpath) => relpath,
            _ => path,
        },
    };
    relpath
        .to_str()
        .ok_or_else(|| internal("path not utf-8"))
        .map(|f| f.replace(" ", "\\ "))
}

fn add_deps_for_unit<'a, 'b>(
    deps: &mut BTreeSet<PathBuf>,
    context: &mut Context<'a, 'b>,
    unit: &Unit<'a>,
    visited: &mut HashSet<Unit<'a>>,
) -> CargoResult<()> {
    if !visited.insert(*unit) {
        return Ok(());
    }

    // units representing the execution of a build script don't actually
    // generate a dep info file, so we just keep on going below
    if !unit.mode.is_run_custom_build() {
        // Add dependencies from rustc dep-info output (stored in fingerprint directory)
        let dep_info_loc = fingerprint::dep_info_loc(context, unit);
        if let Some(paths) = fingerprint::parse_dep_info(
            unit.pkg.root(),
            context.files().host_root(),
            &dep_info_loc,
        )? {
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
    let key = (unit.pkg.package_id(), unit.kind);
    if let Some(output) = context.build_script_outputs.lock().unwrap().get(&key) {
        for path in &output.rerun_if_changed {
            deps.insert(path.into());
        }
    }

    // Recursively traverse all transitive dependencies
    for dep_unit in context.dep_targets(unit).iter() {
        let source_id = dep_unit.pkg.package_id().source_id();
        if source_id.is_path() {
            add_deps_for_unit(deps, context, dep_unit, visited)?;
        }
    }
    Ok(())
}

pub fn output_depinfo<'a, 'b>(cx: &mut Context<'a, 'b>, unit: &Unit<'a>) -> CargoResult<()> {
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
                .ok_or_else(|| internal("build.dep-info-basedir path not utf-8"))?
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
        .filter(|o| o.flavor != FileFlavor::DebugInfo)
    {
        if let Some(ref link_dst) = output.hardlink {
            let output_path = link_dst.with_extension("d");
            if success {
                let target_fn = render_filename(link_dst, basedir)?;

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
