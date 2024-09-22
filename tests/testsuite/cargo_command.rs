//! Tests for custom cargo commands and other global command features.

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str;

use cargo_test_support::basic_manifest;
use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::tools::echo_subcommand;
use cargo_test_support::{
    basic_bin_manifest, cargo_exe, cargo_process, paths, project, project_in_home,
};
use cargo_util::paths::join_paths;

fn path() -> Vec<PathBuf> {
    env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect()
}

#[cargo_test]
fn list_commands_with_descriptions() {
    let p = project().build();
    p.cargo("--list")
        .with_stdout_data(
            "\
...
    b                    alias: build
...
    build                Compile a local package and all of its dependencies
...
    c                    alias: check
...
    r                    alias: run
...
    read-manifest        Print a JSON representation of a Cargo.toml manifest.
...
    t                    alias: test
...
",
        )
        .run();
}

#[cargo_test]
fn list_custom_aliases_with_descriptions() {
    let p = project_in_home("proj")
        .file(
            &paths::home().join(".cargo").join("config"),
            r#"
            [alias]
            myaliasstr = "foo --bar"
            myaliasvec = ["foo", "--bar"]
        "#,
        )
        .build();

    p.cargo("--list")
        .with_stdout_data(str![[r#"
...
    myaliasstr           alias: foo --bar
    myaliasvec           alias: foo --bar
...
"#]])
        .run();
}

#[cargo_test]
fn list_dedupe() {
    let p = project()
        .executable(Path::new("path-test-1").join("cargo-dupe"), "")
        .executable(Path::new("path-test-2").join("cargo-dupe"), "")
        .build();

    let mut path = path();
    path.push(p.root().join("path-test-1"));
    path.push(p.root().join("path-test-2"));
    let path = env::join_paths(path.iter()).unwrap();

    p.cargo("--list")
        .env("PATH", &path)
        .with_stdout_data(str![[r#"
...
    dupe
...
"#]])
        .run();
}

#[cargo_test]
fn list_command_looks_at_path() {
    let proj = project()
        .executable(Path::new("path-test").join("cargo-1"), "")
        .build();

    let mut path = path();
    path.push(proj.root().join("path-test"));
    let path = env::join_paths(path.iter()).unwrap();
    let output = cargo_process("-v --list")
        .env("PATH", &path)
        .exec_with_output()
        .unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(
        output.contains("\n    1                   "),
        "missing 1: {}",
        output
    );
}

#[cfg(windows)]
#[cargo_test]
fn list_command_looks_at_path_case_mismatch() {
    let proj = project()
        .executable(Path::new("path-test").join("cargo-1"), "")
        .build();

    let mut path = path();
    path.push(proj.root().join("path-test"));
    let path = env::join_paths(path.iter()).unwrap();

    // See issue #11814: Environment variable names are case-insensitive on Windows.
    // We need to check that having "Path" instead of "PATH" is okay.
    let output = cargo_process("-v --list")
        .env("Path", &path)
        .env_remove("PATH")
        .exec_with_output()
        .unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(
        output.contains("\n    1                   "),
        "missing 1: {}",
        output
    );
}

#[expect(deprecated)]
#[cargo_test]
fn list_command_handles_known_external_commands() {
    let p = project()
        .executable(Path::new("path-test").join("cargo-fmt"), "")
        .build();

    let fmt_desc = "    fmt                  Formats all bin and lib files of the current crate using rustfmt.";

    // Without path - fmt isn't there
    p.cargo("--list")
        .env("PATH", "")
        .with_stdout_does_not_contain(fmt_desc)
        .run();

    // With path - fmt is there with known description
    let mut path = path();
    path.push(p.root().join("path-test"));
    let path = env::join_paths(path.iter()).unwrap();

    p.cargo("--list")
        .env("PATH", &path)
        .with_stdout_data(str![[r#"
...
    fmt                  Formats all bin and lib files of the current crate using rustfmt.
..."#]])
        .run();
}

#[cargo_test]
fn list_command_resolves_symlinks() {
    let proj = project()
        .symlink(cargo_exe(), Path::new("path-test").join("cargo-2"))
        .build();

    let mut path = path();
    path.push(proj.root().join("path-test"));
    let path = env::join_paths(path.iter()).unwrap();
    let output = cargo_process("-v --list")
        .env("PATH", &path)
        .exec_with_output()
        .unwrap();
    let output = str::from_utf8(&output.stdout).unwrap();
    assert!(
        output.contains("\n    2                   "),
        "missing 2: {}",
        output
    );
}

#[cargo_test]
fn find_closest_capital_c_to_c() {
    cargo_process("C")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `C`

	Did you mean `c`?

	View all installed commands with `cargo --list`
	Find a package to install `C` with `cargo search cargo-C`

"#]])
        .run();
}

#[cargo_test]
fn find_closest_capital_b_to_b() {
    cargo_process("B")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `B`

	Did you mean `b`?

	View all installed commands with `cargo --list`
	Find a package to install `B` with `cargo search cargo-B`

"#]])
        .run();
}

#[cargo_test]
fn find_closest_biuld_to_build() {
    cargo_process("biuld")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `biuld`

	Did you mean `build`?

	View all installed commands with `cargo --list`
	Find a package to install `biuld` with `cargo search cargo-biuld`

"#]])
        .run();

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

    cargo_process("install cargo-biuld").run();
    cargo_process("biuld")
        .with_stdout_data(str![[r#"
Similar, but not identical to, build

"#]])
        .run();
    cargo_process("--list")
        .with_stdout_data(str![[r#"
...
    biuld
...
    build                Compile a local package and all of its dependencies
..."#]])
        .run();
}

#[cargo_test]
fn find_closest_alias() {
    let root = paths::root();
    let my_home = root.join("my_home");
    fs::create_dir(&my_home).unwrap();
    fs::write(
        &my_home.join("config.toml"),
        r#"
            [alias]
            myalias = "build"
        "#,
    )
    .unwrap();

    cargo_process("myalais")
        .env("CARGO_HOME", &my_home)
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `myalais`

	Did you mean `myalias`?

	View all installed commands with `cargo --list`
	Find a package to install `myalais` with `cargo search cargo-myalais`

"#]])
        .run();

    // But, if no alias is defined, it must not suggest one!
    cargo_process("myalais")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `myalais`

	View all installed commands with `cargo --list`
	Find a package to install `myalais` with `cargo search cargo-myalais`

"#]])
        .run();
}

// If a subcommand is more than an edit distance of 3 away, we don't make a suggestion.
#[cargo_test]
fn find_closest_dont_correct_nonsense() {
    cargo_process("there-is-no-way-that-there-is-a-command-close-to-this")
		.cwd(&paths::root())
		.with_status(101)
		.with_stderr_data(str![[r#"
[ERROR] no such command: `there-is-no-way-that-there-is-a-command-close-to-this`

	View all installed commands with `cargo --list`
	Find a package to install `there-is-no-way-that-there-is-a-command-close-to-this` with `cargo search cargo-there-is-no-way-that-there-is-a-command-close-to-this`

"#]])
        .run();
}

#[cargo_test]
fn displays_subcommand_on_error() {
    cargo_process("invalid-command")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `invalid-command`

	View all installed commands with `cargo --list`
	Find a package to install `invalid-command` with `cargo search cargo-invalid-command`

"#]])
        .run();
}

#[cargo_test]
fn override_cargo_home() {
    let root = paths::root();
    let my_home = root.join("my_home");
    fs::create_dir(&my_home).unwrap();
    fs::write(
        &my_home.join("config"),
        r#"
            [cargo-new]
            vcs = "none"
        "#,
    )
    .unwrap();

    cargo_process("new foo").env("CARGO_HOME", &my_home).run();

    assert!(!paths::root().join("foo/.git").is_dir());

    cargo_process("new foo2").run();

    assert!(paths::root().join("foo2/.git").is_dir());
}

#[cargo_test]
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

    let p = project()
        .at("cargo-envtest")
        .file("Cargo.toml", &basic_bin_manifest("cargo-envtest"))
        .file("src/main.rs", &src)
        .build();

    let target_dir = p.target_debug_dir();

    p.cargo("build").run();
    assert!(p.bin("cargo-envtest").is_file());

    let cargo = cargo_exe().canonicalize().unwrap();
    let mut path = path();
    path.push(target_dir.clone());
    let path = env::join_paths(path.iter()).unwrap();

    cargo_process("envtest")
        .env("PATH", &path)
        .with_stdout_data(format!("{}\n", cargo.to_str().unwrap()).raw())
        .run();

    // Check that subcommands inherit an overridden $CARGO
    let envtest_bin = target_dir
        .join("cargo-envtest")
        .with_extension(std::env::consts::EXE_EXTENSION)
        .canonicalize()
        .unwrap();
    let envtest_bin = envtest_bin.to_str().unwrap();
    cargo_process("envtest")
        .env("PATH", &path)
        .env(cargo::CARGO_ENV, &envtest_bin)
        .with_stdout_data(format!("{}\n", envtest_bin).raw().raw())
        .run();
}

#[cargo_test]
fn cargo_cmd_bins_vs_explicit_path() {
    // Set up `cargo-foo` binary in two places: inside `$HOME/.cargo/bin` and outside of it
    //
    // Return paths to both places
    fn set_up_cargo_foo() -> (PathBuf, PathBuf) {
        let p = project()
            .at("cargo-foo")
            .file("Cargo.toml", &basic_manifest("cargo-foo", "1.0.0"))
            .file(
                "src/bin/cargo-foo.rs",
                r#"fn main() { println!("INSIDE"); }"#,
            )
            .file(
                "src/bin/cargo-foo2.rs",
                r#"fn main() { println!("OUTSIDE"); }"#,
            )
            .build();
        p.cargo("build").run();
        let cargo_bin_dir = paths::home().join(".cargo/bin");
        cargo_bin_dir.mkdir_p();
        let root_bin_dir = paths::root().join("bin");
        root_bin_dir.mkdir_p();
        let exe_name = format!("cargo-foo{}", env::consts::EXE_SUFFIX);
        fs::rename(p.bin("cargo-foo"), cargo_bin_dir.join(&exe_name)).unwrap();
        fs::rename(p.bin("cargo-foo2"), root_bin_dir.join(&exe_name)).unwrap();

        (root_bin_dir, cargo_bin_dir)
    }

    let (outside_dir, inside_dir) = set_up_cargo_foo();

    // If `$CARGO_HOME/bin` is not in a path, prefer it over anything in `$PATH`.
    //
    // This is the historical behavior we don't want to break.
    cargo_process("foo")
        .with_stdout_data(str![[r#"
INSIDE

"#]])
        .run();

    // When `$CARGO_HOME/bin` is in the `$PATH`
    // use only `$PATH` so the user-defined ordering is respected.
    {
        cargo_process("foo")
            .env(
                "PATH",
                join_paths(&[&inside_dir, &outside_dir], "PATH").unwrap(),
            )
            .with_stdout_data(str![[r#"
INSIDE

"#]])
            .run();

        cargo_process("foo")
            // Note: trailing slash
            .env(
                "PATH",
                join_paths(&[inside_dir.join(""), outside_dir.join("")], "PATH").unwrap(),
            )
            .with_stdout_data(str![[r#"
INSIDE

"#]])
            .run();

        cargo_process("foo")
            .env(
                "PATH",
                join_paths(&[&outside_dir, &inside_dir], "PATH").unwrap(),
            )
            .with_stdout_data(str![[r#"
OUTSIDE

"#]])
            .run();

        cargo_process("foo")
            // Note: trailing slash
            .env(
                "PATH",
                join_paths(&[outside_dir.join(""), inside_dir.join("")], "PATH").unwrap(),
            )
            .with_stdout_data(str![[r#"
OUTSIDE

"#]])
            .run();
    }
}

#[cargo_test]
fn cargo_subcommand_args() {
    let p = echo_subcommand();
    let cargo_foo_bin = p.bin("cargo-echo");
    assert!(cargo_foo_bin.is_file());

    let mut path = path();
    path.push(p.target_debug_dir());
    let path = env::join_paths(path.iter()).unwrap();

    cargo_process("echo bar -v --help")
        .env("PATH", &path)
        .with_stdout_data(str![[r#"
echo bar -v --help

"#]])
        .run();
}

#[cargo_test]
fn explain() {
    cargo_process("--explain E0001")
        .with_stdout_data(str![[r#"
...
This error suggests that the expression arm corresponding to the noted pattern[..]
...
"#]])
        .run();
}

#[cargo_test]
fn closed_output_ok() {
    // Checks that closed output doesn't cause an error.
    let mut p = cargo_process("--list").build_command();
    p.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = p.spawn().unwrap();
    // Close stdout
    drop(child.stdout.take());
    // Read stderr
    let mut s = String::new();
    child
        .stderr
        .as_mut()
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    let status = child.wait().unwrap();
    assert!(status.success());
    assert!(s.is_empty(), "{}", s);
}

#[cargo_test]
fn subcommand_leading_plus_output_contains() {
    cargo_process("+nightly")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `+nightly`

	Cargo does not handle `+toolchain` directives.
	Did you mean to invoke `cargo` through `rustup` instead?

"#]])
        .run();
}

#[cargo_test]
fn full_did_you_mean() {
    cargo_process("bluid")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `bluid`

	Did you mean `build`?

	View all installed commands with `cargo --list`
	Find a package to install `bluid` with `cargo search cargo-bluid`

"#]])
        .run();
}
