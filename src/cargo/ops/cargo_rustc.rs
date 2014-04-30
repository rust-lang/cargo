use std;
use std::os::args;
use std::io;
use std::io::process::{Process,ProcessConfig,InheritFd};
use std::path::Path;
use {CargoResult,CargoError,ToCargoError,NoFlags,core};
use core;

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

}

fn rustc() {
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
