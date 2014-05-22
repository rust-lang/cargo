use std::os::args;
use std::io;
use std::path::Path;
use std::str;
use core;
use util;
use util::{other_error,human_error,CargoResult,CargoError,ProcessBuilder};
use util::result::ProcessError;

type Args = Vec<String>;

pub fn compile_packages(pkgs: &core::PackageSet) -> CargoResult<()> {
    debug!("compiling; pkgs={}", pkgs);

    let mut sorted = match pkgs.sort() {
        Some(pkgs) => pkgs,
        None => return Err(other_error("circular dependency detected"))
    };

    let root = sorted.pop();

    for pkg in sorted.iter() {
        println!("Compiling {}", pkg);
        try!(compile_pkg(pkg, pkgs, |rustc| rustc.exec_with_output()));
    }

    println!("Compiling {}", root);
    try!(compile_pkg(&root, pkgs, |rustc| rustc.exec()));

    Ok(())
}

fn compile_pkg<T>(pkg: &core::Package, pkgs: &core::PackageSet, exec: |&ProcessBuilder| -> CargoResult<T>) -> CargoResult<()> {
    debug!("compiling; pkg={}; targets={}; deps={}", pkg, pkg.get_targets(), pkg.get_dependencies());
    // Build up the destination
    // let src = pkg.get_root().join(Path::new(pkg.get_source().path.as_slice()));
    let target_dir = pkg.get_absolute_target_dir();

    debug!("creating target dir; path={}", target_dir.display());

    // First ensure that the directory exists
    try!(mk_target(&target_dir).map_err(|_| other_error("could not create target directory")));

    // compile
    for target in pkg.get_targets().iter() {
        try!(rustc(pkg.get_root(), target, &target_dir, pkgs.get_packages(), |rustc| exec(rustc)))
    }

    Ok(())
}

fn mk_target(target: &Path) -> io::IoResult<()> {
    io::fs::mkdir_recursive(target, io::UserRWX)
}

fn rustc<T>(root: &Path, target: &core::Target, dest: &Path, deps: &[core::Package], exec: |&ProcessBuilder| -> CargoResult<T>) -> CargoResult<()> {
    let rustc = prepare_rustc(root, target, dest, deps);

    try!(exec(&rustc)
        .map_err(|err| rustc_to_cargo_err(rustc.get_args().as_slice(), root, err)));

    Ok(())
}

fn prepare_rustc(root: &Path, target: &core::Target, dest: &Path, deps: &[core::Package]) -> ProcessBuilder {
    let mut args = Vec::new();

    build_base_args(&mut args, target, dest);
    build_deps_args(&mut args, deps);

    util::process("rustc")
        .cwd(root.clone())
        .args(args.as_slice())
}

fn build_base_args(into: &mut Args, target: &core::Target, dest: &Path) {
    // TODO: Handle errors in converting paths into args
    into.push(target.get_path().display().to_str());
    into.push("--crate-type".to_str());
    into.push(target.rustc_crate_type().to_str());
    into.push("--out-dir".to_str());
    into.push(dest.display().to_str());
}

fn build_deps_args(dst: &mut Args, deps: &[core::Package]) {
    for dep in deps.iter() {
        let dir = dep.get_absolute_target_dir();

        dst.push("-L".to_str());
        dst.push(dir.display().to_str());
    }
}

fn rustc_to_cargo_err(args: &[String], cwd: &Path, err: CargoError) -> CargoError {
    let msg = {
        let output = match err {
            CargoError { kind: ProcessError(_, ref output), .. } => output,
            _ => fail!("Bug! exec() returned an error other than a ProcessError")
        };

        let mut msg = format!("failed to execute: `rustc {}`", args.connect(" "));

        output.as_ref().map(|o| {
            let second = format!("; Error:\n{}", str::from_utf8_lossy(o.error.as_slice()));
            msg.push_str(second.as_slice());
        });

        msg
    };

    human_error(msg, format!("root={}", cwd.display()), err)
}
