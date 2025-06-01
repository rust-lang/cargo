//! # Cargo test macro.
//!
//! This is meant to be consumed alongside `cargo-test-support`. See
//! <https://rust-lang.github.io/cargo/contrib/> for a guide on writing tests.
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo
//! > and not intended for external use. This
//! > crate may make major changes to its APIs or be deprecated without warning.

use proc_macro::*;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

/// Replacement for `#[test]`
///
/// The `#[cargo_test]` attribute extends `#[test]` with some setup before starting the test.
/// It will create a filesystem "sandbox" under the "cargo integration test" directory for each test, such as `/path/to/cargo/target/tmp/cit/t123/`.
/// The sandbox will contain a `home` directory that will be used instead of your normal home directory.
///
/// The `#[cargo_test]` attribute takes several options that will affect how the test is generated.
/// They are listed in parentheses separated with commas, such as:
///
/// ```rust,ignore
/// #[cargo_test(nightly, reason = "-Zfoo is unstable")]
/// ```
///
/// The options it supports are:
///
/// * `>=1.64` --- This indicates that the test will only run with the given version of `rustc` or newer.
///   This can be used when a new `rustc` feature has been stabilized that the test depends on.
///   If this is specified, a `reason` is required to explain why it is being checked.
/// * `nightly` --- This will cause the test to be ignored if not running on the nightly toolchain.
///   This is useful for tests that use unstable options in `rustc` or `rustdoc`.
///   These tests are run in Cargo's CI, but are disabled in rust-lang/rust's CI due to the difficulty of updating both repos simultaneously.
///   A `reason` field is required to explain why it is nightly-only.
/// * `requires = "<cmd>"` --- This indicates a command that is required to be installed to be run.
///   For example, `requires = "rustfmt"` means the test will only run if the executable `rustfmt` is installed.
///   These tests are *always* run on CI.
///   This is mainly used to avoid requiring contributors from having every dependency installed.
/// * `build_std_real` --- This is a "real" `-Zbuild-std` test (in the `build_std` integration test).
///   This only runs on nightly, and only if the environment variable `CARGO_RUN_BUILD_STD_TESTS` is set (these tests on run on Linux).
/// * `build_std_mock` --- This is a "mock" `-Zbuild-std` test (which uses a mock standard library).
///   This only runs on nightly, and is disabled for windows-gnu.
/// * `public_network_test` --- This tests contacts the public internet.
///   These tests are disabled unless the `CARGO_PUBLIC_NETWORK_TESTS` environment variable is set.
///   Use of this should be *extremely rare*, please avoid using it if possible.
///   The hosts it contacts should have a relatively high confidence that they are reliable and stable (such as github.com), especially in CI.
///   The tests should be carefully considered for developer security and privacy as well.
/// * `container_test` --- This indicates that it is a test that uses Docker.
///   These tests are disabled unless the `CARGO_CONTAINER_TESTS` environment variable is set.
///   This requires that you have Docker installed.
///   The SSH tests also assume that you have OpenSSH installed.
///   These should work on Linux, macOS, and Windows where possible.
///   Unfortunately these tests are not run in CI for macOS or Windows (no Docker on macOS, and Windows does not support Linux images).
///   See [`cargo-test-support::containers`](https://doc.rust-lang.org/nightly/nightly-rustc/cargo_test_support/containers) for more on writing these tests.
/// * `ignore_windows="reason"` --- Indicates that the test should be ignored on windows for the given reason.
#[proc_macro_attribute]
pub fn cargo_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Ideally these options would be embedded in the test itself. However, I
    // find it very helpful to have the test clearly state whether or not it
    // is ignored. It would be nice to have some kind of runtime ignore
    // support (such as
    // https://internals.rust-lang.org/t/pre-rfc-skippable-tests/14611).
    //
    // Unfortunately a big drawback here is that if the environment changes
    // (such as the existence of the `git` CLI), this will not trigger a
    // rebuild and the test will still be ignored. In theory, something like
    // `tracked_env` or `tracked_path`
    // (https://github.com/rust-lang/rust/issues/99515) could help with this,
    // but they don't really handle the absence of files well.
    let mut ignore = false;
    let mut requires_reason = false;
    let mut explicit_reason = None;
    let mut implicit_reasons = Vec::new();
    macro_rules! set_ignore {
        ($predicate:expr, $($arg:tt)*) => {
            let p = $predicate;
            ignore |= p;
            if p {
                implicit_reasons.push(std::fmt::format(format_args!($($arg)*)));
            }
        };
    }
    let is_not_nightly = !version().1;
    for rule in split_rules(attr) {
        match rule.as_str() {
            "build_std_real" => {
                // Only run the "real" build-std tests on nightly and with an
                // explicit opt-in (these generally only work on linux, and
                // have some extra requirements, and are slow, and can pollute
                // the environment since it downloads dependencies).
                set_ignore!(is_not_nightly, "requires nightly");
                set_ignore!(
                    option_env!("CARGO_RUN_BUILD_STD_TESTS").is_none(),
                    "CARGO_RUN_BUILD_STD_TESTS must be set"
                );
            }
            "build_std_mock" => {
                // Only run the "mock" build-std tests on nightly and disable
                // for windows-gnu which is missing object files (see
                // https://github.com/rust-lang/wg-cargo-std-aware/issues/46).
                set_ignore!(is_not_nightly, "requires nightly");
                set_ignore!(
                    cfg!(all(target_os = "windows", target_env = "gnu")),
                    "does not work on windows-gnu"
                );
            }
            "container_test" => {
                // These tests must be opt-in because they require docker.
                set_ignore!(
                    option_env!("CARGO_CONTAINER_TESTS").is_none(),
                    "CARGO_CONTAINER_TESTS must be set"
                );
            }
            "public_network_test" => {
                // These tests must be opt-in because they touch the public
                // network. The use of these should be **EXTREMELY RARE**, and
                // should only touch things which would nearly certainly work
                // in CI (like github.com).
                set_ignore!(
                    option_env!("CARGO_PUBLIC_NETWORK_TESTS").is_none(),
                    "CARGO_PUBLIC_NETWORK_TESTS must be set"
                );
            }
            "nightly" => {
                requires_reason = true;
                set_ignore!(is_not_nightly, "requires nightly");
            }
            "requires_rustup_stable" => {
                set_ignore!(
                    !has_rustup_stable(),
                    "rustup or stable toolchain not installed"
                );
            }
            s if s.starts_with("requires=") => {
                let command = &s[9..];
                let Ok(literal) = command.parse::<Literal>() else {
                    panic!("expect a string literal, found: {command}");
                };
                let literal = literal.to_string();
                let Some(command) = literal
                    .strip_prefix('"')
                    .and_then(|lit| lit.strip_suffix('"'))
                else {
                    panic!("expect a quoted string literal, found: {literal}");
                };
                set_ignore!(!has_command(command), "{command} not installed");
            }
            s if s.starts_with(">=1.") => {
                requires_reason = true;
                let min_minor = s[4..].parse().unwrap();
                let minor = version().0;
                set_ignore!(minor < min_minor, "requires rustc 1.{minor} or newer");
            }
            s if s.starts_with("reason=") => {
                explicit_reason = Some(s[7..].parse().unwrap());
            }
            s if s.starts_with("ignore_windows=") => {
                set_ignore!(cfg!(windows), "{}", &s[16..s.len() - 1]);
            }
            _ => panic!("unknown rule {:?}", rule),
        }
    }
    if requires_reason && explicit_reason.is_none() {
        panic!(
            "#[cargo_test] with a rule also requires a reason, \
            such as #[cargo_test(nightly, reason = \"needs -Z unstable-thing\")]"
        );
    }

    // Construct the appropriate attributes.
    let span = Span::call_site();
    let mut ret = TokenStream::new();
    let add_attr = |ret: &mut TokenStream, attr_name, attr_input| {
        ret.extend(Some(TokenTree::from(Punct::new('#', Spacing::Alone))));
        let attr = TokenTree::from(Ident::new(attr_name, span));
        let mut attr_stream: TokenStream = attr.into();
        if let Some(input) = attr_input {
            attr_stream.extend(input);
        }
        ret.extend(Some(TokenTree::from(Group::new(
            Delimiter::Bracket,
            attr_stream,
        ))));
    };
    add_attr(&mut ret, "test", None);
    if ignore {
        let reason = explicit_reason
            .or_else(|| {
                (!implicit_reasons.is_empty())
                    .then(|| TokenTree::from(Literal::string(&implicit_reasons.join(", "))).into())
            })
            .map(|reason: TokenStream| {
                let mut stream = TokenStream::new();
                stream.extend(Some(TokenTree::from(Punct::new('=', Spacing::Alone))));
                stream.extend(Some(reason));
                stream
            });
        add_attr(&mut ret, "ignore", reason);
    }

    // Find where the function body starts, and add the boilerplate at the start.
    for token in item {
        let group = match token {
            TokenTree::Group(g) => {
                if g.delimiter() == Delimiter::Brace {
                    g
                } else {
                    ret.extend(Some(TokenTree::Group(g)));
                    continue;
                }
            }
            other => {
                ret.extend(Some(other));
                continue;
            }
        };

        let mut new_body = to_token_stream(
            r#"let _test_guard = {
                let tmp_dir = option_env!("CARGO_TARGET_TMPDIR");
                cargo_test_support::paths::init_root(tmp_dir)
            };"#,
        );

        new_body.extend(group.stream());
        ret.extend(Some(TokenTree::from(Group::new(
            group.delimiter(),
            new_body,
        ))));
    }

    ret
}

fn split_rules(t: TokenStream) -> Vec<String> {
    let tts: Vec<_> = t.into_iter().collect();
    tts.split(|tt| match tt {
        TokenTree::Punct(p) => p.as_char() == ',',
        _ => false,
    })
    .filter(|parts| !parts.is_empty())
    .map(|parts| {
        parts
            .into_iter()
            .map(|part| part.to_string())
            .collect::<String>()
    })
    .collect()
}

fn to_token_stream(code: &str) -> TokenStream {
    code.parse().unwrap()
}

static VERSION: std::sync::LazyLock<(u32, bool)> = LazyLock::new(|| {
    let output = Command::new("rustc")
        .arg("-V")
        .output()
        .expect("rustc should run");
    let stdout = std::str::from_utf8(&output.stdout).expect("utf8");
    let vers = stdout.split_whitespace().skip(1).next().unwrap();
    let is_nightly = option_env!("CARGO_TEST_DISABLE_NIGHTLY").is_none()
        && (vers.contains("-nightly") || vers.contains("-dev"));
    let minor = vers.split('.').skip(1).next().unwrap().parse().unwrap();
    (minor, is_nightly)
});

fn version() -> (u32, bool) {
    LazyLock::force(&VERSION).clone()
}

fn check_command(command_path: &Path, args: &[&str]) -> bool {
    let mut command = Command::new(command_path);
    let command_name = command.get_program().to_str().unwrap().to_owned();
    command.args(args);
    let output = match command.output() {
        Ok(output) => output,
        Err(e) => {
            // * hg is not installed on GitHub macOS or certain constrained
            //   environments like Docker. Consider installing it if Cargo
            //   gains more hg support, but otherwise it isn't critical.
            // * lldb is not pre-installed on Ubuntu and Windows, so skip.
            if is_ci() && !matches!(command_name.as_str(), "hg" | "lldb") {
                panic!("expected command `{command_name}` to be somewhere in PATH: {e}",);
            }
            return false;
        }
    };
    if !output.status.success() {
        panic!(
            "expected command `{command_name}` to be runnable, got error {}:\n\
            stderr:{}\n\
            stdout:{}\n",
            output.status,
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }
    true
}

fn has_command(command: &str) -> bool {
    use std::env::consts::EXE_EXTENSION;
    // ALLOWED: For testing cargo itself only.
    #[allow(clippy::disallowed_methods)]
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths)
        .flat_map(|path| {
            let candidate = path.join(&command);
            let with_exe = if EXE_EXTENSION.is_empty() {
                None
            } else {
                Some(candidate.with_extension(EXE_EXTENSION))
            };
            std::iter::once(candidate).chain(with_exe)
        })
        .find(|p| is_executable(p))
        .is_some()
}

#[cfg(unix)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    use std::os::unix::prelude::*;
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().is_file()
}

fn has_rustup_stable() -> bool {
    if option_env!("CARGO_TEST_DISABLE_NIGHTLY").is_some() {
        // This cannot run on rust-lang/rust CI due to the lack of rustup.
        return false;
    }
    // Cargo mucks with PATH on Windows, adding sysroot host libdir, which is
    // "bin", which circumvents the rustup wrapper. Use the path directly from
    // CARGO_HOME.
    let home = match option_env!("CARGO_HOME") {
        Some(home) => home,
        None if is_ci() => panic!("expected to run under rustup"),
        None => return false,
    };
    let cargo = Path::new(home).join("bin/cargo");
    check_command(&cargo, &["+stable", "--version"])
}

/// Whether or not this running in a Continuous Integration environment.
fn is_ci() -> bool {
    // Consider using `tracked_env` instead of option_env! when it is stabilized.
    // `tracked_env` will handle changes, but not require rebuilding the macro
    // itself like option_env does.
    option_env!("CI").is_some() || option_env!("TF_BUILD").is_some()
}
