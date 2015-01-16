use std::io::fs;
use std::io;
use std::io::{USER_RWX, File};
use std::os;
use std::str;
use cargo::util::process;

use support::paths;
use support::{execs, project, cargo_dir, mkdir_recursive, ProjectBuilder};
use hamcrest::{assert_that};

fn setup() {
}

/// Add an empty file with executable flags (and platform-dependent suffix).
/// TODO: move this to `ProjectBuilder` if other cases using this emerge.
fn fake_executable(proj: ProjectBuilder, dir: &Path, name: &str) -> ProjectBuilder {
    let path = proj.root().join(dir).join(format!("{}{}", name, os::consts::EXE_SUFFIX));
    mkdir_recursive(&Path::new(path.dirname())).unwrap();
    fs::File::create(&path).unwrap();
    let io::FileStat{perm, ..} = fs::stat(&path).unwrap();
    fs::chmod(&path, io::OTHER_EXECUTE | perm).unwrap();
    proj
}

fn path() -> Vec<Path> {
    let path = os::getenv_as_bytes("PATH").unwrap_or(Vec::new());
    os::split_paths(path)
}
test!(list_commands_looks_at_path {
    let proj = project("list-non-overlapping");
    let proj = fake_executable(proj, &Path::new("path-test"), "cargo-1");
    let pr = process(cargo_dir().join("cargo"))
        .unwrap()
        .cwd(proj.root())
        .env("HOME", Some(paths::home()));

    let mut path = path();
    path.push(proj.root().join("path-test"));
    let path = os::join_paths(path.as_slice()).unwrap();
    let output = pr.arg("-v").arg("--list").env("PATH", Some(path.as_slice()));
    let output = output.exec_with_output().unwrap();
    let output = str::from_utf8(output.output.as_slice()).unwrap();
    assert!(output.contains("\n    1\n"), "missing 1: {}", output);
});

test!(find_closest_biuld_to_build {
    let pr = process(cargo_dir().join("cargo")).unwrap()
                    .arg("biuld").cwd(paths::root())
                    .env("HOME", Some(paths::home()));

    assert_that(pr,
                execs().with_status(127)
                       .with_stderr("No such subcommand

Did you mean `build`?

"));
});

// if a subcommand is more than 3 edit distance away, we don't make a suggestion
test!(find_closest_dont_correct_nonsense {
    let pr = process(cargo_dir().join("cargo")).unwrap()
                    .arg("asdf").cwd(paths::root())
                    .env("HOME", Some(paths::home()));

    assert_that(pr,
                execs().with_status(127)
                       .with_stderr("No such subcommand
"));
});

test!(override_cargo_home {
    let root = paths::root();
    let my_home = root.join("my_home");
    fs::mkdir(&my_home, USER_RWX).unwrap();
    File::create(&my_home.join("config")).write_str(r#"
        [cargo-new]
        name = "foo"
        email = "bar"
        git = false
    "#).unwrap();

    assert_that(process(cargo_dir().join("cargo")).unwrap()
                .arg("new").arg("foo")
                .cwd(paths::root())
                .env("USER", Some("foo"))
                .env("HOME", Some(paths::home()))
                .env("CARGO_HOME", Some(my_home.clone())),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
    let toml = File::open(&toml).read_to_string().unwrap();
    assert!(toml.as_slice().contains(r#"authors = ["foo <bar>"]"#));
});
