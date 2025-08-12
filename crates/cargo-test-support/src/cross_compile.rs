//! Support for cross-compile tests with the `--target` flag.
//!
//! Note that cross-testing is very limited. You need to install the
//! "alternate" target to the host (32-bit for 64-bit hosts or vice-versa).
//!
//! Set `CFG_DISABLE_CROSS_TESTS=1` environment variable to disable these tests
//! if you are unable to use the alternate target. Unfortunately 32-bit
//! support on macOS is going away, so macOS users are out of luck.
//!
//! These tests are all disabled on rust-lang/rust's CI, but run in Cargo's CI.

use std::env;

/// The arch triple of the test-running host.
pub fn native() -> &'static str {
    env!("NATIVE_ARCH")
}

pub fn native_arch() -> &'static str {
    match native()
        .split("-")
        .next()
        .expect("Target triple has unexpected format")
    {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        "i686" => "x86",
        _ => panic!("This test should be gated on cross_compile::disabled."),
    }
}

/// The alternate target-triple to build with.
///
/// Only use this function on tests that check `cross_compile::disabled`.
pub fn alternate() -> &'static str {
    try_alternate().expect("This test should be gated on cross_compile::disabled.")
}

/// A possible alternate target-triple to build with.
pub(crate) fn try_alternate() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("x86_64-apple-darwin")
    } else if cfg!(target_os = "linux") {
        Some("i686-unknown-linux-gnu")
    } else if cfg!(all(target_os = "windows", target_env = "msvc")) {
        Some("i686-pc-windows-msvc")
    } else if cfg!(all(target_os = "windows", target_env = "gnu")) {
        Some("i686-pc-windows-gnu")
    } else {
        None
    }
}

pub fn alternate_arch() -> &'static str {
    if cfg!(target_os = "macos") {
        "x86_64"
    } else {
        "x86"
    }
}

/// A target-triple that is neither the host nor the target.
///
/// Rustc may not work with it and it's alright, apart from being a
/// valid target triple it is supposed to be used only as a
/// placeholder for targets that should not be considered.
pub fn unused() -> &'static str {
    "wasm32-unknown-unknown"
}

/// Check if the given target has been installed.
///
/// Generally `testsuite::utils::cross_compile::disabled` should be used to check if cross-compilation is allowed.
/// And [`alternate`] to get the cross target.
///
/// You should only use this as a last resort to skip tests,
/// because it doesn't report skipped tests as ignored.
pub fn requires_target_installed(target: &str) -> bool {
    let has_target = std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .ok()
        .map(|output| {
            String::from_utf8(output.stdout)
                .map(|stdout| stdout.contains(target))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if !has_target {
        let msg =
            format!("to run this test, run `rustup target add {target} --toolchain <toolchain>`",);
        if cargo_util::is_ci() {
            panic!("{msg}");
        } else {
            eprintln!("{msg}");
        }
    }
    has_target
}
