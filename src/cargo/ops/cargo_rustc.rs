use std;
use std::os::args;
use std::io;
use std::io::process::{Process,ProcessConfig,InheritFd};
use std::path::Path;
use core::errors::{CLIError,CLIResult,ToResult};
use NoFlags;
use core;
use util;

type Args = Vec<~str>;

pub fn compile(pkgs: &core::PackageSet) -> CLIResult<()> {
    let sorted = match pkgs.sort() {
        Some(pkgs) => pkgs,
        None => return Err(CLIError::new("Circular dependency detected", None, 1))
    };

    for pkg in sorted.iter() {
        try!(compile_pkg(pkg, pkgs));
    }

    Ok(())
}


fn compile_pkg(pkg: &core::Package, pkgs: &core::PackageSet) -> CLIResult<()> {
    // Build up the destination
    let src = pkg.get_root().join(Path::new(pkg.get_source().path.as_slice()));
    let target = pkg.get_root().join(Path::new(pkg.get_target()));

    // First ensure that the directory exists
    try!(mk_target(&target).to_result(|err|
        CLIError::new(format!("Could not create the target directory {}", target.display()), Some(err.to_str()), 1)));

    // compile
    try!(rustc(pkg.get_root(), &src, &target, deps(pkg, pkgs)));

    Ok(())
}

fn mk_target(target: &Path) -> io::IoResult<()> {
    io::fs::mkdir_recursive(target, io::UserRWX)
}

fn rustc(root: &Path, src: &Path, target: &Path, deps: &[core::Package]) -> CLIResult<()> {
    let mut args = Vec::new();

    build_base_args(&mut args, src, target);
    build_deps_args(&mut args, deps);

    try!(util::process("rustc")
        .cwd(root.clone())
        .args(args.as_slice())
        .exec()
        .to_result(|err|
            CLIError::new(format!("Couldn't execute rustc {}", args.connect(" ")), Some(err.to_str()), 1)));

    Ok(())
}

fn build_base_args(dst: &mut Args, src: &Path, target: &Path) {
    dst.push(src.as_str().unwrap().to_owned());
    dst.push("--crate-type".to_owned());
    dst.push("lib".to_owned());
    dst.push("--out-dir".to_owned());
    dst.push(target.as_str().unwrap().to_owned());
}

fn build_deps_args(dst: &mut Args, deps: &[core::Package]) {
    for dep in deps.iter() {
        let target = dep.get_root().join(Path::new(dep.get_target()));

        dst.push("-L".to_owned());
        dst.push(target.as_str().unwrap().to_owned());
    }
}

// Collect all dependencies for a given package
fn deps(pkg: &core::Package, pkgs: &core::PackageSet) -> ~[core::Package] {
    let names: ~[&str] = pkg.get_dependencies().iter().map(|d| d.get_name()).collect();
    pkgs.get_all(names).iter().map(|p| (*p).clone()).collect()
}

pub fn execute(_: NoFlags, manifest: core::Manifest) -> CLIResult<Option<core::Manifest>> {
    let core::Manifest { root, lib, bin, .. } = manifest;

    let (crate_type, out_dir) = if lib.len() > 0 {
        ( "lib".to_owned(), lib[0].path )
    } else if bin.len() > 0 {
        ( "bin".to_owned(), bin[0].path )
    } else {
        return Err(CLIError::new("bad manifest, no lib or bin specified", None, 1));
    };

    let root = Path::new(root);
    let target = join(&root, "target".to_owned());

    let args = [
        join(&root, out_dir),
        "--out-dir".to_owned(), target,
        "--crate-type".to_owned(), crate_type
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

    let mut p = try!(Process::configure(config).to_result(|err|
        CLIError::new(format!("Could not start process: rustc {}", args.connect(" ")), Some(err.to_str()), 1)));

    let status = p.wait();

    if status != std::io::process::ExitStatus(0) {
        return Err(CLIError::new(format!("Non-zero status code from rustc {}", args.connect(" ")), None, 1));
    }

    Ok(None)
}

fn join(path: &Path, part: ~str) -> ~str {
    format!("{}", path.join(part).display())
}
