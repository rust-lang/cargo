use std::os::args;
use std::io;
use std::path::Path;
use core;
use util;
use util::{other_error,CargoResult,CargoError};

type Args = Vec<~str>;

pub fn compile(pkgs: &core::PackageSet) -> CargoResult<()> {
    let sorted = match pkgs.sort() {
        Some(pkgs) => pkgs,
        None => return Err(other_error("circular dependency detected"))
    };

    for pkg in sorted.iter() {
        println!("Compiling {}", pkg);
        try!(compile_pkg(pkg, pkgs));
    }

    Ok(())
}

fn compile_pkg(pkg: &core::Package, pkgs: &core::PackageSet) -> CargoResult<()> {
    // Build up the destination
    // let src = pkg.get_root().join(Path::new(pkg.get_source().path.as_slice()));
    let target_dir = pkg.get_absolute_target_dir();

    // First ensure that the directory exists
    try!(mk_target(&target_dir).map_err(|_| other_error("could not create target directory")));

    // compile
    for target in pkg.get_targets().iter() {
        try!(rustc(pkg.get_root(), target, &target_dir, pkgs.get_packages()))
    }

    Ok(())
}

fn mk_target(target: &Path) -> io::IoResult<()> {
    io::fs::mkdir_recursive(target, io::UserRWX)
}

fn rustc(root: &Path, target: &core::Target, dest: &Path, deps: &[core::Package]) -> CargoResult<()> {
    let mut args = Vec::new();

    build_base_args(&mut args, target, dest);
    build_deps_args(&mut args, deps);

    try!(util::process("rustc")
        .cwd(root.clone())
        .args(args.as_slice())
        .exec()
        .map_err(|err| rustc_to_cargo_err(&args, root, err)));

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

fn rustc_to_cargo_err(args: &Vec<~str>, cwd: &Path, err: io::IoError) -> CargoError {
    other_error("failed to exec rustc")
        .with_detail(format!("args={}; root={}; cause={}", args.connect(" "), cwd.display(), err.to_str()))
}
