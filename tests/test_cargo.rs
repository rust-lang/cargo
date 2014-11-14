use std::io::fs::{mod, PathExtensions};
use std::io;
use std::os;
use std::str;
use cargo::util::process;

use support::paths;
use support::{execs, project, cargo_dir, mkdir_recursive, ProjectBuilder, ResultTest};
use hamcrest::{assert_that};

fn setup() {
}

/// Add an empty file with executable flags (and platform-dependent suffix).
/// TODO: move this to `ProjectBuilder` if other cases using this emerge.
fn fake_executable(proj: ProjectBuilder, dir: &Path, name: &str) -> ProjectBuilder {
    let path = proj.root().join(dir).join(format!("{}{}", name, os::consts::EXE_SUFFIX));
    mkdir_recursive(&Path::new(path.dirname())).assert();
    fs::File::create(&path).assert();
    let io::FileStat{perm, ..} = fs::stat(&path).assert();
    fs::chmod(&path, io::OTHER_EXECUTE | perm).assert();
    proj
}

// We can't entirely obliterate PATH because windows needs it for paths to
// things like libgcc, but we want to filter out everything which has a `cargo`
// installation as we don't want it to muck with the --list tests
fn new_path() -> Vec<Path> {
    let path = os::getenv_as_bytes("PATH").unwrap_or(Vec::new());
    os::split_paths(path).into_iter().filter(|p| {
        !p.join(format!("cargo{}", os::consts::EXE_SUFFIX)).exists()
    }).collect()
}
test!(list_commands_looks_at_path {
    let proj = project("list-non-overlapping");
    let proj = fake_executable(proj, &Path::new("path-test"), "cargo-1");
    let pr = process(cargo_dir().join("cargo")).cwd(proj.root())
                    .env("HOME", Some(paths::home()));

    let mut path = new_path();
    path.push(proj.root().join("path-test"));
    let path = os::join_paths(path.as_slice()).unwrap();
    let output = pr.arg("-v").arg("--list").env("PATH", Some(path.as_slice()));
    let output = output.exec_with_output().assert();
    let output = str::from_utf8(output.output.as_slice()).assert();
    assert!(output.contains("\n    1\n"), "missing 1: {}", output);
})

test!(find_closest_biuld_to_build {
    let pr = process(cargo_dir().join("cargo"))
                    .arg("biuld").cwd(paths::root())
                    .env("HOME", Some(paths::home()));

    assert_that(pr,
                execs().with_status(127)
                       .with_stderr("No such subcommand

Did you mean `build`?

"));
})

// if a subcommand is more than 3 edit distance away, we don't make a suggestion
test!(find_closest_dont_correct_nonsense {
    let pr = process(cargo_dir().join("cargo"))
                    .arg("asdf").cwd(paths::root())
                    .env("HOME", Some(paths::home()));

    assert_that(pr,
                execs().with_status(127)
                       .with_stderr("No such subcommand
"));
})
