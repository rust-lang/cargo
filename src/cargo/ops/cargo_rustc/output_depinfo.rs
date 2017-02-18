use std::collections::HashSet;
use std::io::{Write, BufWriter, ErrorKind};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use ops::{Context, Unit};
use util::{CargoResult, internal};
use ops::cargo_rustc::fingerprint;

fn render_filename<P: AsRef<Path>>(path: P, basedir: Option<&str>) -> CargoResult<String> {
    let path = path.as_ref();
    let relpath = match basedir {
        None => path,
        Some(base) => match path.strip_prefix(base) {
            Ok(relpath) => relpath,
            _ => path,
        }
    };
    relpath.to_str().ok_or(internal("path not utf-8")).map(|f| f.replace(" ", "\\ "))
}

fn add_deps_for_unit<'a, 'b>(deps: &mut HashSet<PathBuf>, context: &mut Context<'a, 'b>,
    unit: &Unit<'a>, visited: &mut HashSet<Unit<'a>>) -> CargoResult<()>
{
    if !visited.insert(*unit) {
        return Ok(());
    }

    // Add dependencies from rustc dep-info output (stored in fingerprint directory)
    let dep_info_loc = fingerprint::dep_info_loc(context, unit);
    if let Some(paths) = fingerprint::parse_dep_info(&dep_info_loc)? {
        for path in paths {
            deps.insert(path);
        }
    } else {
        debug!("can't find dep_info for {:?} {:?}",
            unit.pkg.package_id(), unit.profile);
        return Err(internal("dep_info missing"));
    }

    // Add rerun-if-changed dependencies
    let key = (unit.pkg.package_id().clone(), unit.kind);
    if let Some(output) = context.build_state.outputs.lock().unwrap().get(&key) {
        for path in &output.rerun_if_changed {
            deps.insert(path.into());
        }
    }

    // Recursively traverse all transitive dependencies
    for dep_unit in &context.dep_targets(unit)? {
        let source_id = dep_unit.pkg.package_id().source_id();
        if source_id.is_path() {
            add_deps_for_unit(deps, context, dep_unit, visited)?;
        }
    }
    Ok(())
}

pub fn output_depinfo<'a, 'b>(context: &mut Context<'a, 'b>, unit: &Unit<'a>) -> CargoResult<()> {
    let mut deps = HashSet::new();
    let mut visited = HashSet::new();
    let success = add_deps_for_unit(&mut deps, context, unit, &mut visited).is_ok();
    let basedir = None; // TODO
    for (_filename, link_dst, _linkable) in context.target_filenames(unit)? {
        if let Some(link_dst) = link_dst {
            let output_path = link_dst.with_extension("d");
            if success {
                let mut outfile = BufWriter::new(File::create(output_path)?);
                let target_fn = render_filename(link_dst, basedir)?;
                write!(outfile, "{}:", target_fn)?;
                for dep in &deps {
                    write!(outfile, " {}", render_filename(dep, basedir)?)?;
                }
                writeln!(outfile, "")?;
            } else {
                // dep-info generation failed, so delete output file. This will usually
                // cause the build system to always rerun the build rule, which is correct
                // if inefficient.
                if let Err(err) = fs::remove_file(output_path) {
                    if err.kind() != ErrorKind::NotFound {
                        return Err(err.into());
                    }
                }
            }
        }
    }
    Ok(())
}
