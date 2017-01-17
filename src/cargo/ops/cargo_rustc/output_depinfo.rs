use std::borrow::ToOwned;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::fs::File;
use std::path::{Path, PathBuf};

use core::{TargetKind};
use ops::{Context, Unit};
use util::{CargoResult, internal, human};

#[derive(Debug)]
struct DepLine {
    target: String,
    deps: Vec<String>,
}

struct DepFile {
    dir: String,
    deps: Vec<DepLine>,
}

fn render_filename<P: AsRef<Path>>(path: P, basedir: Option<&str>) -> CargoResult<String> {
    let path = path.as_ref();
    let relpath = match basedir {
        None => path,
        Some(base) => match path.strip_prefix(base) {
            Ok(relpath) => relpath,
            _ => path,
        }
    };
    relpath.to_str().ok_or(internal("path not utf-8")).map(ToOwned::to_owned)
}

fn read_dep_file<P: AsRef<Path>>(path: P) -> CargoResult<DepFile> {
    let mut file = File::open(&path).map_err(|_|
        human("error opening ".to_string() + path.as_ref().to_str().unwrap_or("(bad unicode")))?;
    let mut contents = String::new();
    let _ = file.read_to_string(&mut contents)?;
    let mut spl = contents.split('\0');
    let dir = spl.next().ok_or(internal("dependency file empty"))?;
    let dep_txt = spl.next().ok_or(internal("dependency file missing null byte"))?;
    let mut result = Vec::new();
    for line in dep_txt.lines() {
        let mut line_spl = line.split(": ");
        if let Some(target) = line_spl.next() {
            if let Some(deps) = line_spl.next() {
                let deps = deps.split_whitespace().map(ToOwned::to_owned).collect();
                result.push(DepLine {
                    target: target.to_string(),
                    deps: deps,
                });
            }
        }
    }
    Ok(DepFile {
        dir: dir.to_string(),
        deps: result,
    })
}

fn add_deps(depfile: &DepFile, deps: &mut HashSet<PathBuf>) {
    let dep_dir = PathBuf::from(&depfile.dir);
    for depline in &depfile.deps {
        for dep in &depline.deps {
            deps.insert(dep_dir.join(dep));
        }
    }
}

// TODO: probably better to use Context::target_filenames for this
fn target_filename(context: &mut Context, unit: &Unit) -> CargoResult<PathBuf> {
    let (dir, base) = context.link_stem(&unit).ok_or(internal("can't get link stem"))?;
    if unit.target.is_lib() {
        Ok(dir.join(["lib", &base, ".rlib"].concat()))
    } else {
        Ok(dir.join(base))
    }
}

fn add_deps_for_unit(deps: &mut HashSet<PathBuf>, context: &mut Context, unit: &Unit)
    -> CargoResult<()>
{
    // TODO: this is duplicated against filename in fingerprint.rs
    let kind = match *unit.target.kind() {
        TargetKind::Lib(..) => "lib",
        TargetKind::Bin => "bin",
        TargetKind::Test => "integration-test",
        TargetKind::Example => "example",
        TargetKind::Bench => "bench",
        TargetKind::CustomBuild => "build-script",
    };
    let flavor = if unit.profile.test {
        "test-"
    } else if unit.profile.doc {
        "doc-"
    } else {
        ""
    };
    let dep_filename = ["dep-", flavor, kind, "-", &context.file_stem(&unit)].concat();
    let path = context.fingerprint_dir(&unit).join(&dep_filename);
    let depfile = read_dep_file(&path)?;
    add_deps(&depfile, deps);
    Ok(())
}

pub fn output_depinfo(context: &mut Context, unit: &Unit) -> CargoResult<()> {
    let mut deps = HashSet::new();
    add_deps_for_unit(&mut deps, context, unit)?;
    for dep_unit in &context.dep_targets(unit)? {
        let source_id = dep_unit.pkg.package_id().source_id();
        if source_id.is_path() {
            add_deps_for_unit(&mut deps, context, dep_unit)?;
        }
    }
    let filename = target_filename(context, unit)?;
    let mut output_path = filename.clone().into_os_string();
    output_path.push(".d");
    let basedir = None; // TODO
    let target_fn = render_filename(filename, basedir)?;
    let mut outfile = File::create(output_path)?;
    write!(outfile, "{}:", target_fn)?;
    for dep in &deps {
        write!(outfile, " {}", render_filename(dep, basedir)?)?;
    }
    writeln!(outfile, "")?;
    Ok(())
}
