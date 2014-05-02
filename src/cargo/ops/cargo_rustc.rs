use std;
use std::os::args;
use std::io;
use std::io::process::{Process,ProcessConfig,InheritFd};
use std::path::Path;
use {CargoResult,CargoError,ToCargoError,NoFlags,core};
use core;
use util;

type Args = Vec<~str>;

pub fn compile(pkgs: &core::PackageSet) {
    let sorted = match pkgs.sort() {
        Some(pkgs) => pkgs,
        None => return
    };

    for pkg in sorted.iter() {
        compile_pkg(pkg, pkgs);
    }
}


fn compile_pkg(pkg: &core::Package, pkgs: &core::PackageSet) {
    // Build up the destination
    let src = pkg.get_root().join(Path::new(pkg.get_source().name.as_slice()));
    let target = pkg.get_root().join(Path::new(pkg.get_target()));

    // First ensure that the directory exists
    mk_target(&target);

    // compile
    rustc(pkg.get_root(), &src, &target, deps(pkg, pkgs));
}

fn mk_target(target: &Path) -> io::IoResult<()> {
    io::fs::mkdir_recursive(target, io::UserRWX)
}

fn rustc(root: &Path, src: &Path, target: &Path, deps: &[core::Package]) {
    let mut args = Vec::new();

    build_base_args(&mut args, src, target);
    build_deps_args(&mut args, deps);

    util::process("rustc")
        .cwd(root.clone())
        .args(args.as_slice())
        .exec();
}

fn build_base_args(dst: &mut Args, src: &Path, target: &Path) {
    dst.push(src.as_str().unwrap().to_owned());
    dst.push(~"--crate-type");
    dst.push(~"lib");
    dst.push(~"--out-dir");
    dst.push(target.as_str().unwrap().to_owned());
}

fn build_deps_args(dst: &mut Args, deps: &[core::Package]) {
    for dep in deps.iter() {
        let target = dep.get_root().join(Path::new(dep.get_target()));

        dst.push(~"-L");
        dst.push(target.as_str().unwrap().to_owned());
    }
}

// Collect all dependencies for a given package
fn deps(pkg: &core::Package, pkgs: &core::PackageSet) -> ~[core::Package] {
    let names: ~[&str] = pkg.get_dependencies().iter().map(|d| d.get_name()).collect();
    pkgs.get_all(names).iter().map(|p| (*p).clone()).collect()
}

pub fn execute(_: NoFlags, manifest: core::Manifest) -> CargoResult<Option<core::Manifest>> {
    let core::Manifest { root, lib, bin, .. } = manifest;

    let (crate_type, out_dir) = if lib.len() > 0 {
        ( ~"lib", lib[0].path )
    } else if bin.len() > 0 {
        ( ~"bin", bin[0].path )
    } else {
        return Err(CargoError::new(~"bad manifest, no lib or bin specified", 1));
    };

    let root = Path::new(root);
    let target = join(&root, ~"target");

    let args = [
        join(&root, out_dir),
        ~"--out-dir", target,
        ~"--crate-type", crate_type
    ];

    match io::fs::mkdir_recursive(&root.join("target"), io::UserRWX) {
        Err(_) => fail!("Couldn't mkdir -p"),
        Ok(val) => val
    }

    println!("Executing rustc {}", args.as_slice());

    let mut config = ProcessConfig::new();
    config.stdout = InheritFd(1);
    config.stderr = InheritFd(2);
    config.program = "rustc";
    config.args = args.as_slice();

    let mut p = try!(Process::configure(config).to_cargo_error(format!("Could not start process: rustc {}", args.as_slice()), 1));

    let status = p.wait();

    if status != std::io::process::ExitStatus(0) {
        fail!("Failed to execute")
    }

    Ok(None)
}

fn join(path: &Path, part: ~str) -> ~str {
    format!("{}", path.join(part).display())
}
