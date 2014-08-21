use std::io::fs;
use std::io;
use std::os;
use std::str;
use cargo::util::{process, ProcessBuilder};

use support::paths;
use support::{project, cargo_dir, mkdir_recursive, ProjectBuilder, ResultTest};

fn setup() {
}

/// Add an empty file with executable flags (and platform-dependent suffix).
/// TODO: move this to `ProjectBuilder` if other cases using this emerge.
fn fake_executable(proj: ProjectBuilder, dir: &Path, name: &str) -> ProjectBuilder {
    let path = proj.root().join(dir).join(format!("{}{}", name, os::consts::EXE_SUFFIX));
    mkdir_recursive(&Path::new(path.dirname())).assert();
    fs::File::create(&path).assert();
    let io::FileStat{perm, ..} = fs::stat(&path).assert();
    fs::chmod(&path, io::OtherExecute | perm).assert();
    proj
}

/// Copy real cargo exeutable just built to specified location, and
/// prepare to run it.
fn copied_executable_process(proj: &ProjectBuilder, name: &str, dir: &Path) -> ProcessBuilder {
    let name = format!("{}{}", name, os::consts::EXE_SUFFIX);
    let path_src = cargo_dir().join(name.clone());
    let path_dst = proj.root().join(dir).join(name);
    mkdir_recursive(&Path::new(path_dst.dirname())).assert();
    fs::copy(&path_src, &path_dst).assert();
    process(path_dst)
        .cwd(proj.root())
        .env("HOME", Some(paths::home().as_vec()))
}

// We can't entirely obliterate PATH because windows needs it for paths to
// things like libgcc, but we want to filter out everything which has a `cargo`
// installation as we don't want it to muck with the --list tests
fn new_path() -> Vec<Path> {
    let path = os::getenv_as_bytes("PATH").unwrap_or(Vec::new());
    os::split_paths(path).move_iter().filter(|p| {
        !p.join(format!("cargo{}", os::consts::EXE_SUFFIX)).exists()
    }).collect()
}
test!(list_commands_non_overlapping {
    // lib/cargo | cargo-3
    // bin/       | cargo-2
    // PATH       | cargo-1
    // Check if --list searches all 3 targets.
    // Also checks that results are in lexicographic order.
    let proj = project("list-non-overlapping");
    let proj = fake_executable(proj, &Path::new("lib/cargo"), "cargo-3");
    let proj = fake_executable(proj, &Path::new("bin"), "cargo-2");
    let proj = fake_executable(proj, &Path::new("path-test"), "cargo-1");
    let pr = copied_executable_process(&proj, "cargo", &Path::new("bin"));

    let mut path = new_path();
    path.push(proj.root().join("path-test"));
    let path = os::join_paths(path.as_slice()).unwrap();
    let output = pr.arg("-v").arg("--list").env("PATH", Some(path.as_slice()));
    let output = output.exec_with_output().assert();
    let output = str::from_utf8(output.output.as_slice()).assert();
    assert!(output.contains("\n    1\n"), "missing 1: {}", output);
    assert!(output.contains("\n    2\n"), "missing 2: {}", output);
    assert!(output.contains("\n    3\n"), "missing 3: {}", output);
})
