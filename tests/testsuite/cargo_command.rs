use std::env;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::str;

use cargo;
use support::cargo_process;
use support::paths::{self, CargoPathExt};
use support::registry::Package;
use support::{basic_manifest, basic_bin_manifest, cargo_exe, execs, project, Project};
use support::hamcrest::{assert_that, existing_file};

#[cfg_attr(windows, allow(dead_code))]
enum FakeKind<'a> {
    Executable,
    Symlink { target: &'a Path },
}

/// Add an empty file with executable flags (and platform-dependent suffix).
/// TODO: move this to `Project` if other cases using this emerge.
fn fake_file(proj: Project, dir: &Path, name: &str, kind: &FakeKind) -> Project {
    let path = proj.root()
        .join(dir)
        .join(&format!("{}{}", name, env::consts::EXE_SUFFIX));
    path.parent().unwrap().mkdir_p();
    match *kind {
        FakeKind::Executable => {
            File::create(&path).unwrap();
            make_executable(&path);
        }
        FakeKind::Symlink { target } => {
            make_symlink(&path, target);
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
        ::std::os::unix::fs::symlink(t, p).expect("Failed to create symlink");
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
fn list_commands_with_descriptions() {
    let p = project().build();
    let output = p.cargo("--list").exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(
        output.contains("\n    build                Compile a local package and all of its dependencies"),
        "missing build, with description: {}",
        output
    );
    // assert read-manifest prints the right one-line description followed by another command, indented.
    assert!(
        output.contains("\n    read-manifest        Print a JSON representation of a Cargo.toml manifest.\n    "),
        "missing build, with description: {}",
        output
    );
}

#[test]
fn list_command_looks_at_path() {
    let proj = project().build();
    let proj = fake_file(
        proj,
        Path::new("path-test"),
        "cargo-1",
        &FakeKind::Executable,
    );

    let mut path = path();
    path.push(proj.root().join("path-test"));
    let path = env::join_paths(path.iter()).unwrap();
    let mut p = cargo_process("-v --list");
    let output = p.env("PATH", &path);
    let output = output.exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(
        output.contains("\n    1                   "),
        "missing 1: {}",
        output
    );
}

// windows and symlinks don't currently agree that well
#[cfg(unix)]
#[test]
fn list_command_resolves_symlinks() {
    let proj = project().build();
    let proj = fake_file(
        proj,
        Path::new("path-test"),
        "cargo-2",
        &FakeKind::Symlink {
            target: &cargo_exe(),
        },
    );

    let mut path = path();
    path.push(proj.root().join("path-test"));
    let path = env::join_paths(path.iter()).unwrap();
    let mut p = cargo_process("-v --list");
    let output = p.env("PATH", &path);
    let output = output.exec_with_output().unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(
        output.contains("\n    2                   "),
        "missing 2: {}",
        output
    );
}

#[test]
fn find_closest_biuld_to_build() {
    assert_that(
        cargo_process("biuld"),
        execs().with_status(101).with_stderr_contains(
            "\
error: no such subcommand: `biuld`

<tab>Did you mean `build`?
",
        ),
    );

    // But, if we actually have `biuld`, it must work!
    // https://github.com/rust-lang/cargo/issues/5201
    Package::new("cargo-biuld", "1.0.0")
        .file(
            "src/main.rs",
            r#"
            fn main() {
                println!("Similar, but not identical to, build");
            }
        "#,
        )
        .publish();

    assert_that(
        cargo_process("install cargo-biuld"),
        execs(),
    );
    assert_that(
        cargo_process("biuld"),
        execs()
            .with_stdout("Similar, but not identical to, build\n"),
    );
    assert_that(
        cargo_process("--list"),
        execs()
            .with_stdout_contains("    build                Compile a local package and all of its dependencies\n")
            .with_stdout_contains("    biuld\n"),
    );
}

// if a subcommand is more than 3 edit distance away, we don't make a suggestion
#[test]
fn find_closest_dont_correct_nonsense() {
    assert_that(
        cargo_process("there-is-no-way-that-there-is-a-command-close-to-this")
            .cwd(&paths::root()),
        execs().with_status(101).with_stderr(
            "[ERROR] no such subcommand: \
                        `there-is-no-way-that-there-is-a-command-close-to-this`
",
        ),
    );
}

#[test]
fn displays_subcommand_on_error() {
    assert_that(
        cargo_process("invalid-command"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] no such subcommand: `invalid-command`\n"),
    );
}

#[test]
fn override_cargo_home() {
    let root = paths::root();
    let my_home = root.join("my_home");
    fs::create_dir(&my_home).unwrap();
    File::create(&my_home.join("config"))
        .unwrap()
        .write_all(
            br#"
        [cargo-new]
        name = "foo"
        email = "bar"
        git = false
    "#,
        )
        .unwrap();

    assert_that(
        cargo_process("new foo").env("USER", "foo").env("CARGO_HOME", &my_home),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["foo <bar>"]"#));
}

#[test]
fn cargo_subcommand_env() {
    let src = format!(
        r#"
        use std::env;

        fn main() {{
            println!("{{}}", env::var("{}").unwrap());
        }}
        "#,
        cargo::CARGO_ENV
    );

    let p = project().at("cargo-envtest")
        .file("Cargo.toml", &basic_bin_manifest("cargo-envtest"))
        .file("src/main.rs", &src)
        .build();

    let target_dir = p.target_debug_dir();

    assert_that(p.cargo("build"), execs());
    assert_that(&p.bin("cargo-envtest"), existing_file());

    let cargo = cargo_exe().canonicalize().unwrap();
    let mut path = path();
    path.push(target_dir);
    let path = env::join_paths(path.iter()).unwrap();

    assert_that(
        cargo_process("envtest").env("PATH", &path),
        execs().with_stdout(cargo.to_str().unwrap()),
    );
}

#[test]
fn cargo_subcommand_args() {
    let p = project().at("cargo-foo")
        .file("Cargo.toml", &basic_manifest("cargo-foo", "0.0.1"))
        .file(
            "src/main.rs",
            r#"
            fn main() {
                let args: Vec<_> = ::std::env::args().collect();
                println!("{:?}", args);
            }
        "#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
    let cargo_foo_bin = p.bin("cargo-foo");
    assert_that(&cargo_foo_bin, existing_file());

    let mut path = path();
    path.push(p.target_debug_dir());
    let path = env::join_paths(path.iter()).unwrap();

    assert_that(
        cargo_process("foo bar -v --help").env("PATH", &path),
        execs().with_stdout(format!(
            r#"[{:?}, "foo", "bar", "-v", "--help"]"#,
            cargo_foo_bin
        )),
    );
}

#[test]
fn cargo_help() {
    assert_that(cargo_process(""), execs());
    assert_that(cargo_process("help"), execs());
    assert_that(cargo_process("-h"), execs());
    assert_that(cargo_process("help build"), execs());
    assert_that(cargo_process("build -h"), execs());
    assert_that(cargo_process("help help"), execs());
}

#[test]
fn cargo_help_external_subcommand() {
    Package::new("cargo-fake-help", "1.0.0")
        .file(
            "src/main.rs",
            r#"
            fn main() {
                if ::std::env::args().nth(2) == Some(String::from("--help")) {
                    println!("fancy help output");
                }
            }"#,
            )
            .publish();
    assert_that(
        cargo_process("install cargo-fake-help"),
        execs(),
    );
    assert_that(
        cargo_process("help fake-help"),
        execs().with_stdout("fancy help output\n")
    );
}

#[test]
fn explain() {
    assert_that(
        cargo_process("--explain E0001"),
        execs().with_stdout_contains(
            "This error suggests that the expression arm corresponding to the noted pattern",
        ),
    );
}

// Test that the output of 'cargo -Z help' shows a different help screen with
// all the -Z flags.
#[test]
fn z_flags_help() {
    assert_that(
        cargo_process("-Z help"),
        execs().with_stdout_contains(
            "    -Z unstable-options -- Allow the usage of unstable options such as --registry",
        ),
    );
}
