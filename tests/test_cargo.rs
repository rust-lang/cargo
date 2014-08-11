use cargo::util::{process, ProcessBuilder};
use hamcrest::{assert_that};
use std::io;
use std::io::fs;
use std::os;
use support::paths;
use support::{project, execs, cargo_dir, mkdir_recursive, ProjectBuilder, ResultTest};

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

test!(list_commands_empty {
    let proj = project("list-runs");
    let pr = copied_executable_process(&proj, "cargo", &Path::new("bin")).arg("--list");
    assert_that(pr, execs()
        .with_status(0)
        .with_stdout("Installed Commands:\n"));
})

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
    let pr = copied_executable_process(&proj, "cargo", &Path::new("bin")).arg("--list");

    let path_test = proj.root().join("path-test");
    // On Windows, cargo.exe seems to require some directory (
    // I don't know which) to run properly.
    // That's why we append to $PATH here, instead of overwriting.
    let path = os::getenv_as_bytes("PATH").unwrap();
    let mut components = os::split_paths(path);
    components.push(path_test);
    let path_var = os::join_paths(components.as_slice()).assert();
    assert_that(
        pr.env("PATH", Some(path_var.as_slice())),
        execs()
            .with_status(0)
            .with_stdout("Installed Commands:\n   1\n   2\n   3\n"));
})
