use std::os::args;
use std::io;
use std::path::Path;
use core::errors::{CLIError,CLIResult,ToResult};
use core;
use util;

type Args = Vec<~str>;

pub fn compile(pkgs: &core::PackageSet) -> CLIResult<()> {
    let sorted = match pkgs.sort() {
        Some(pkgs) => pkgs,
        None => return Err(CLIError::new("Circular dependency detected", None, 1))
    };

    for pkg in sorted.iter() {
        println!("Compiling {}", pkg);
        try!(compile_pkg(pkg, pkgs));
    }

    Ok(())
}

fn compile_pkg(pkg: &core::Package, pkgs: &core::PackageSet) -> CLIResult<()> {
    // Build up the destination
    // let src = pkg.get_root().join(Path::new(pkg.get_source().path.as_slice()));
    let target_dir = pkg.get_absolute_target_dir();

    // First ensure that the directory exists
    try!(mk_target(&target_dir).to_result(|err|
        CLIError::new(format!("Could not create the target directory {}", target_dir.display()), Some(err.to_str()), 1)));

    // compile
    for target in pkg.get_targets().iter() {
        try!(rustc(pkg.get_root(), target, &target_dir, deps(pkg, pkgs)));
    }

    Ok(())
}

fn mk_target(target: &Path) -> io::IoResult<()> {
    io::fs::mkdir_recursive(target, io::UserRWX)
}

fn rustc(root: &Path, target: &core::Target, dest: &Path, deps: &[core::Package]) -> CLIResult<()> {
    let mut args = Vec::new();

    build_base_args(&mut args, target, dest);
    build_deps_args(&mut args, deps);

    try!(util::process("rustc")
        .cwd(root.clone())
        .args(args.as_slice())
        .exec()
        .to_result(|err|
            CLIError::new(format!("Couldn't execute `rustc {}` in `{}`", args.connect(" "), root.display()), Some(err.to_str()), 1)));

    Ok(())
}

fn build_base_args(into: &mut Args, target: &core::Target, dest: &Path) {
    into.push(target.get_path().as_str().unwrap().to_owned());
    into.push("--crate-type".to_owned());
    into.push(target.rustc_crate_type().to_owned());
    into.push("--out-dir".to_owned());
    into.push(dest.as_str().unwrap().to_owned());
}

fn build_deps_args(dst: &mut Args, deps: &[core::Package]) {
    for dep in deps.iter() {
        let dir = dep.get_absolute_target_dir();

        dst.push("-L".to_owned());
        dst.push(dir.as_str().unwrap().to_owned());
    }
}

// Collect all dependencies for a given package
fn deps(pkg: &core::Package, pkgs: &core::PackageSet) -> ~[core::Package] {
    let names: ~[&str] = pkg.get_dependencies().iter().map(|d| d.get_name()).collect();
    pkgs.get_all(names).iter().map(|p| (*p).clone()).collect()
}
