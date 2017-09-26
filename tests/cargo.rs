extern crate cargo;
extern crate cargotest;
extern crate hamcrest;

use std::env;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::str;

use cargotest::cargo_process;
use cargotest::support::paths::{self, CargoPathExt};
use cargotest::support::{execs, project, ProjectBuilder, basic_bin_manifest};
use hamcrest::{assert_that, existing_file};

#[cfg_attr(windows,allow(dead_code))]
enum FakeKind<'a> {
    Executable,
    Symlink{target:&'a Path},
}

/// Add an empty file with executable flags (and platform-dependent suffix).
/// TODO: move this to `ProjectBuilder` if other cases using this emerge.
fn fake_file(proj: ProjectBuilder, dir: &Path, name: &str, kind: &FakeKind) -> ProjectBuilder {
    let path = proj.root().join(dir).join(&format!("{}{}", name,
                                                   env::consts::EXE_SUFFIX));
    path.parent().unwrap().mkdir_p();
    match *kind {
        FakeKind::Executable => {
            File::create(&path).unwrap();
            make_executable(&path);
        },
        FakeKind::Symlink{target} => {
            make_symlink(&path,target);
        }
    }
    return proj;

    #[cfg(unix)]
    fn make_executable(p: &Path) {
        use std::os::unix::prelude::*;

        let mut perms = fs::metadata(p).unwrap().permissions();
        let mode = perms.mode();
        perms.set_mode(mode | 0o111);
        fs::set_permissions(p, perms).unwrap();
    }
    #[cfg(windows)]
    fn make_executable(_: &Path) {}
    #[cfg(unix)]
    fn make_symlink(p: &Path, t: &Path) {
        ::std::os::unix::fs::symlink(t,p).expect("Failed to create symlink");
    }
    #[cfg(windows)]
    fn make_symlink(_: &Path, _: &Path) {
        panic!("Not supported")
    }
}

fn path() -> Vec<PathBuf> {
    env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect()
}

#[test]
fn list_command_looks_at_path() {
    let proj = project("list-non-overlapping");
    let proj = fake_file(proj, Path::new("path-test"), "cargo-1", &FakeKind::Executable);
    let mut pr = cargo_process();

    let mut path = path();
    path.push(proj.root().join("path-test"));
    let path = env::join_paths(path.iter()).unwrap();
    let output = pr.arg("-v").arg("--list")
                   .env("PATH", &path);
    let output = output.exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(output.contains("\n    1\n"), "missing 1: {}", output);
}

// windows and symlinks don't currently agree that well
#[cfg(unix)]
#[test]
fn list_command_resolves_symlinks() {
    use cargotest::support::cargo_exe;

    let proj = project("list-non-overlapping");
    let proj = fake_file(proj, Path::new("path-test"), "cargo-2",
                         &FakeKind::Symlink{target:&cargo_exe()});
    let mut pr = cargo_process();

    let mut path = path();
    path.push(proj.root().join("path-test"));
    let path = env::join_paths(path.iter()).unwrap();
    let output = pr.arg("-v").arg("--list")
                   .env("PATH", &path);
    let output = output.exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(output.contains("\n    2\n"), "missing 2: {}", output);
}

#[test]
fn find_closest_biuld_to_build() {
    let mut pr = cargo_process();
    pr.arg("biuld");

    assert_that(pr,
                execs().with_status(101)
                       .with_stderr("[ERROR] no such subcommand: `biuld`

<tab>Did you mean `build`?

"));
}

// if a subcommand is more than 3 edit distance away, we don't make a suggestion
#[test]
fn find_closest_dont_correct_nonsense() {
    let mut pr = cargo_process();
    pr.arg("there-is-no-way-that-there-is-a-command-close-to-this")
      .cwd(&paths::root());

    assert_that(pr,
                execs().with_status(101)
                       .with_stderr("[ERROR] no such subcommand: \
                        `there-is-no-way-that-there-is-a-command-close-to-this`
"));
}

#[test]
fn displays_subcommand_on_error() {
    let mut pr = cargo_process();
    pr.arg("invalid-command");

    assert_that(pr,
                execs().with_status(101)
                       .with_stderr("[ERROR] no such subcommand: `invalid-command`
"));
}

#[test]
fn override_cargo_home() {
    let root = paths::root();
    let my_home = root.join("my_home");
    fs::create_dir(&my_home).unwrap();
    File::create(&my_home.join("config")).unwrap().write_all(br#"
        [cargo-new]
        name = "foo"
        email = "bar"
        git = false
    "#).unwrap();

    assert_that(cargo_process()
                    .arg("new").arg("foo")
                    .env("USER", "foo")
                    .env("CARGO_HOME", &my_home),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["foo <bar>"]"#));
}

#[test]
fn cargo_subcommand_env() {
    use cargotest::support::cargo_exe;

    let src = format!(r#"
        use std::env;

        fn main() {{
            println!("{{}}", env::var("{}").unwrap());
        }}
        "#, cargo::CARGO_ENV);

    let p = project("cargo-envtest")
        .file("Cargo.toml", &basic_bin_manifest("cargo-envtest"))
        .file("src/main.rs", &src);

    let target_dir = p.target_debug_dir();

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("cargo-envtest"), existing_file());

    let mut pr = cargo_process();
    let cargo = cargo_exe().canonicalize().unwrap();
    let mut path = path();
    path.push(target_dir);
    let path = env::join_paths(path.iter()).unwrap();

    assert_that(pr.arg("envtest").env("PATH", &path),
                execs().with_status(0).with_stdout(cargo.to_str().unwrap()));
}

#[test]
fn cargo_help() {
    assert_that(cargo_process(),
                execs().with_status(0));
    assert_that(cargo_process().arg("help"),
                execs().with_status(0));
    assert_that(cargo_process().arg("-h"),
                execs().with_status(0));
    assert_that(cargo_process().arg("help").arg("build"),
                execs().with_status(0));
    assert_that(cargo_process().arg("build").arg("-h"),
                execs().with_status(0));
    assert_that(cargo_process().arg("help").arg("-h"),
                execs().with_status(0));
    assert_that(cargo_process().arg("help").arg("help"),
                execs().with_status(0));
}

#[test]
fn explain() {
    assert_that(cargo_process().arg("--explain").arg("E0001"),
                execs().with_status(0));
}
