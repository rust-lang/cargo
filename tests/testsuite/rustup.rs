//! Tests for Cargo's behavior under Rustup.

use std::env;
use std::env::consts::EXE_EXTENSION;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::install::assert_has_installed_exe;
use cargo_test_support::paths::{cargo_home, home, root};
use cargo_test_support::registry::Package;
use cargo_test_support::{execs, process, project, str};

fn pkg(name: &str, vers: &str) {
    Package::new(name, vers)
        .file("src/main.rs", "fn main() {{}}")
        .publish();
}

/// Helper to generate an executable.
fn make_exe(dest: &Path, name: &str, contents: &str, env: &[(&str, PathBuf)]) -> PathBuf {
    let rs_name = format!("{name}.rs");
    fs::write(
        root().join(&rs_name),
        &format!("fn main() {{ {contents} }}"),
    )
    .unwrap();
    let mut pb = process("rustc");
    env.iter().for_each(|(key, value)| {
        pb.env(key, value);
    });
    pb.arg("--edition=2021")
        .arg(root().join(&rs_name))
        .exec()
        .unwrap();
    let exe = Path::new(name).with_extension(EXE_EXTENSION);
    let output = dest.join(&exe);
    fs::rename(root().join(&exe), &output).unwrap();
    output
}

fn prepend_path(path: &Path) -> OsString {
    let mut paths = vec![path.to_path_buf()];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    env::join_paths(paths).unwrap()
}

struct RustupEnvironment {
    /// Path for ~/.cargo/bin
    cargo_bin: PathBuf,
    /// Path for ~/.rustup
    rustup_home: PathBuf,
    /// Path to the cargo executable in the toolchain directory
    /// (~/.rustup/toolchain/test-toolchain/bin/cargo.exe).
    cargo_toolchain_exe: PathBuf,
}

/// Creates an executable which prints a message and then runs the *real* rustc.
fn real_rustc_wrapper(bin_dir: &Path, message: &str) -> PathBuf {
    let real_rustc = cargo_util::paths::resolve_executable("rustc".as_ref()).unwrap();
    // The toolchain rustc needs to call the real rustc. In order to do that,
    // it needs to restore or clear the RUSTUP environment variables so that
    // if rustup is installed, it will call the correct rustc.
    let rustup_toolchain_source_setup = match std::env::var_os("RUSTUP_TOOLCHAIN_SOURCE") {
        Some(t) => format!(
            ".env(\"RUSTUP_TOOLCHAIN_SOURCE\", \"{}\")",
            t.into_string().unwrap()
        ),
        None => format!(".env_remove(\"RUSTUP_TOOLCHAIN_SOURCE\")"),
    };
    let rustup_toolchain_setup = match std::env::var_os("RUSTUP_TOOLCHAIN") {
        Some(t) => format!(
            ".env(\"RUSTUP_TOOLCHAIN\", \"{}\")",
            t.into_string().unwrap()
        ),
        None => format!(".env_remove(\"RUSTUP_TOOLCHAIN\")"),
    };
    let mut env = vec![("CARGO_RUSTUP_TEST_real_rustc", real_rustc)];
    let rustup_home_setup = match std::env::var_os("RUSTUP_HOME") {
        Some(h) => {
            env.push(("CARGO_RUSTUP_TEST_RUSTUP_HOME", h.into()));
            format!(".env(\"RUSTUP_HOME\", env!(\"CARGO_RUSTUP_TEST_RUSTUP_HOME\"))")
        }
        None => format!(".env_remove(\"RUSTUP_HOME\")"),
    };
    make_exe(
        bin_dir,
        "rustc",
        &format!(
            r#"
                eprintln!("{message}");
                let r = std::process::Command::new(env!("CARGO_RUSTUP_TEST_real_rustc"))
                    .args(std::env::args_os().skip(1))
                    {rustup_toolchain_source_setup}
                    {rustup_toolchain_setup}
                    {rustup_home_setup}
                    .status();
                std::process::exit(r.unwrap().code().unwrap_or(2));
            "#
        ),
        &env,
    )
}

/// Creates a simulation of a rustup environment with `~/.cargo/bin` and
/// `~/.rustup` directories populated with some executables that simulate
/// rustup.
///
/// Arguments
///
/// - `proxy_calls_cargo`: if true, the cargo proxy calls the cargo under test;
///    otherwise, the cargo proxy calls an executable that panics immediately
/// - `env_setup`: environment variable setup the proxy should perform
fn simulated_rustup_environment(proxy_calls_cargo: bool, env_setup: &str) -> RustupEnvironment {
    // Set up ~/.rustup/toolchains/test-toolchain/bin with a custom rustc and cargo.
    let rustup_home = home().join(".rustup");
    let toolchain_bin = rustup_home
        .join("toolchains")
        .join("test-toolchain")
        .join("bin");
    toolchain_bin.mkdir_p();
    let rustc_toolchain_exe = real_rustc_wrapper(&toolchain_bin, "real rustc running");
    let cargo_toolchain_exe = if proxy_calls_cargo {
        crate::utils::cargo_exe()
    } else {
        make_exe(
            &toolchain_bin,
            "cargo",
            r#"panic!("cargo toolchain should not be called");"#,
            &[],
        )
    };

    // Set up ~/.cargo/bin with a typical set of rustup proxies.
    let cargo_bin = home().join(".cargo").join("bin");
    cargo_bin.mkdir_p();

    let proxy = make_exe(
        &cargo_bin,
        "rustc",
        &format!(
            r#"
                let file_stem = std::path::PathBuf::from(std::env::args().next().unwrap())
                    .file_stem()
                    .map(ToOwned::to_owned)
                    .unwrap();
                let program = match file_stem.to_str().unwrap() {{
                    "cargo" => env!("CARGO_RUSTUP_TEST_cargo_toolchain_exe"),
                    "rustc" => env!("CARGO_RUSTUP_TEST_rustc_toolchain_exe"),
                    arg => panic!("proxy only supports cargo and rustc, got {{arg:?}}"),
                }};
                eprintln!("`{{program}}` proxy running");
                let r = std::process::Command::new(program)
                    .args(std::env::args_os().skip(1))
                    {env_setup}
                    .status();
                std::process::exit(r.unwrap().code().unwrap_or(2));
            "#
        ),
        &[
            ("CARGO_RUSTUP_TEST_rustc_toolchain_exe", rustc_toolchain_exe),
            (
                "CARGO_RUSTUP_TEST_cargo_toolchain_exe",
                cargo_toolchain_exe.clone(),
            ),
        ],
    );
    fs::hard_link(
        &proxy,
        cargo_bin.join("cargo").with_extension(EXE_EXTENSION),
    )
    .unwrap();
    fs::hard_link(
        &proxy,
        cargo_bin.join("rustup").with_extension(EXE_EXTENSION),
    )
    .unwrap();

    RustupEnvironment {
        cargo_bin,
        rustup_home,
        cargo_toolchain_exe,
    }
}

#[cargo_test]
fn typical_rustup() {
    // Test behavior under a typical rustup setup with a normal toolchain.
    let RustupEnvironment {
        cargo_bin,
        rustup_home,
        cargo_toolchain_exe,
    } = simulated_rustup_environment(false, "");

    // Set up a project and run a normal cargo build.
    let p = project().file("src/lib.rs", "").build();
    // The path is modified so that cargo will call `rustc` from
    // `~/.cargo/bin/rustc to use our custom rustup proxies.
    let path = prepend_path(&cargo_bin);
    p.cargo("check")
        .env("RUSTUP_TOOLCHAIN_SOURCE", "default")
        .env("RUSTUP_TOOLCHAIN", "test-toolchain")
        .env("RUSTUP_HOME", &rustup_home)
        .env("PATH", &path)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
real rustc running
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Do a similar test, but with a toolchain link that does not have cargo
    // (which normally would do a fallback to nightly/beta/stable).
    cargo_toolchain_exe.rm_rf();
    p.build_dir().rm_rf();

    p.cargo("check")
        .env("RUSTUP_TOOLCHAIN_SOURCE", "default")
        .env("RUSTUP_TOOLCHAIN", "test-toolchain")
        .env("RUSTUP_HOME", &rustup_home)
        .env("PATH", &path)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
real rustc running
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

// This doesn't work on Windows because Cargo forces the PATH to contain the
// sysroot_libdir, which is actually `bin`, preventing the test from
// overriding the bin directory.
#[cargo_test(ignore_windows = "PATH can't be overridden on Windows")]
fn custom_calls_other_cargo() {
    // Test behavior when a custom subcommand tries to manipulate PATH to use
    // a different toolchain.
    let RustupEnvironment {
        cargo_bin,
        rustup_home,
        cargo_toolchain_exe: _,
    } = simulated_rustup_environment(false, "");

    // Create a directory with a custom toolchain (outside of the rustup universe).
    let custom_bin = root().join("custom-bin");
    custom_bin.mkdir_p();
    // `cargo` points to the real cargo.
    let cargo_exe = crate::utils::cargo_exe();
    fs::hard_link(&cargo_exe, custom_bin.join(cargo_exe.file_name().unwrap())).unwrap();
    // `rustc` executes the real rustc.
    real_rustc_wrapper(&custom_bin, "custom toolchain rustc running");

    // A project that cargo-custom will try to build.
    let p = project().file("src/lib.rs", "").build();

    // Create a custom cargo subcommand.
    // This will modify PATH to a custom toolchain and call cargo from that.
    make_exe(
        &cargo_bin,
        "cargo-custom",
        r#"
            use std::env;
            use std::process::Command;

            eprintln!("custom command running");

            let mut paths = vec![std::path::PathBuf::from(env!("CARGO_RUSTUP_TEST_custom_bin"))];
            paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
            let path = env::join_paths(paths).unwrap();

            let status = Command::new("cargo")
                .arg("check")
                .current_dir(env!("CARGO_RUSTUP_TEST_project_dir"))
                .env("PATH", path)
                .status()
                .unwrap();
            assert!(status.success());
        "#,
        &[
            ("CARGO_RUSTUP_TEST_custom_bin", custom_bin),
            ("CARGO_RUSTUP_TEST_project_dir", p.root()),
        ],
    );

    cargo_process("custom")
        // Set these to simulate what would happen when running under rustup.
        // We want to make sure that cargo-custom does not try to use the
        // rustup proxies.
        .env("RUSTUP_TOOLCHAIN_SOURCE", "default")
        .env("RUSTUP_TOOLCHAIN", "test-toolchain")
        .env("RUSTUP_HOME", &rustup_home)
        .with_stderr_data(str![[r#"
custom command running
[CHECKING] foo v0.0.1 ([ROOT]/foo)
custom toolchain rustc running
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

/// Performs a `cargo install` with a non-default toolchain in a simulated
/// rustup environment. The purpose is to verify the warning that is emitted.
#[cargo_test]
fn cargo_install_with_non_default_toolchain() {
    let RustupEnvironment {
        cargo_bin,
        rustup_home: _,
        cargo_toolchain_exe: _,
    } = simulated_rustup_environment(
        true,
        ".env(\"RUSTUP_TOOLCHAIN_SOURCE\", \"env\")
         .env(\"RUSTUP_TOOLCHAIN\", \"test-toolchain\")",
    );

    pkg("foo", "0.0.1");

    let mut p = process(cargo_bin.join("cargo"));
    p.arg_line("install foo");
    execs()
        .with_process_builder(p)
        .with_stderr_data(str![[r#"
`[..]/cargo[EXE]` proxy running
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[WARNING] using non-default toolchain `test-toolchain` overridden by env
  |
  = [HELP] use `cargo +stable install` if you meant to use the stable toolchain.
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(cargo_home(), "foo");
}
