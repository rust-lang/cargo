//! Support for cross-compile tests with the `--target` flag.
//!
//! Note that cross-testing is very limited. You need to install the
//! "alternate" target to the host (32-bit for 64-bit hosts or vice-versa).
//!
//! Set CFG_DISABLE_CROSS_TESTS=1 environment variable to disable these tests
//! if you are unable to use the alternate target. Unfortunately 32-bit
//! support on macOS is going away, so macOS users are out of luck.
//!
//! These tests are all disabled on rust-lang/rust's CI, but run in Cargo's CI.

use crate::{basic_manifest, main_file, project};
use cargo_util::ProcessError;
use std::env;
use std::fmt::Write;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

/// Whether or not the resulting cross binaries can run on the host.
static CAN_RUN_ON_HOST: AtomicBool = AtomicBool::new(false);

pub fn disabled() -> bool {
    // First, disable if requested.
    match env::var("CFG_DISABLE_CROSS_TESTS") {
        Ok(ref s) if *s == "1" => return true,
        _ => {}
    }

    // Cross tests are only tested to work on macos, linux, and MSVC windows.
    if !(cfg!(target_os = "macos") || cfg!(target_os = "linux") || cfg!(target_env = "msvc")) {
        return true;
    }

    // It's not particularly common to have a cross-compilation setup, so
    // try to detect that before we fail a bunch of tests through no fault
    // of the user.
    static CAN_BUILD_CROSS_TESTS: AtomicBool = AtomicBool::new(false);
    static CHECK: Once = Once::new();

    let cross_target = alternate();

    let run_cross_test = || -> anyhow::Result<Output> {
        let p = project()
            .at("cross_test")
            .file("Cargo.toml", &basic_manifest("cross_test", "1.0.0"))
            .file("src/main.rs", &main_file(r#""testing!""#, &[]))
            .build();

        let build_result = p
            .cargo("build --target")
            .arg(&cross_target)
            .exec_with_output();

        if build_result.is_ok() {
            CAN_BUILD_CROSS_TESTS.store(true, Ordering::SeqCst);
        }

        let result = p
            .cargo("run --target")
            .arg(&cross_target)
            .exec_with_output();

        if result.is_ok() {
            CAN_RUN_ON_HOST.store(true, Ordering::SeqCst);
        }
        build_result
    };

    CHECK.call_once(|| {
        drop(run_cross_test());
    });

    if CAN_BUILD_CROSS_TESTS.load(Ordering::SeqCst) {
        // We were able to compile a simple project, so the user has the
        // necessary `std::` bits installed. Therefore, tests should not
        // be disabled.
        return false;
    }

    // We can't compile a simple cross project. We want to warn the user
    // by failing a single test and having the remainder of the cross tests
    // pass. We don't use `std::sync::Once` here because panicking inside its
    // `call_once` method would poison the `Once` instance, which is not what
    // we want.
    static HAVE_WARNED: AtomicBool = AtomicBool::new(false);

    if HAVE_WARNED.swap(true, Ordering::SeqCst) {
        // We are some other test and somebody else is handling the warning.
        // Just disable the current test.
        return true;
    }

    // We are responsible for warning the user, which we do by panicking.
    let mut message = format!(
        "
Cannot cross compile to {}.

This failure can be safely ignored. If you would prefer to not see this
failure, you can set the environment variable CFG_DISABLE_CROSS_TESTS to \"1\".

Alternatively, you can install the necessary libraries to enable cross
compilation tests. Cross compilation tests depend on your host platform.
",
        cross_target
    );

    if cfg!(target_os = "linux") {
        message.push_str(
            "
Linux cross tests target i686-unknown-linux-gnu, which requires the ability to
build and run 32-bit targets. This requires the 32-bit libraries to be
installed. For example, on Ubuntu, run `sudo apt install gcc-multilib` to
install the necessary libraries.
",
        );
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        message.push_str(
            "
macOS on aarch64 cross tests to target x86_64-apple-darwin.
This should be natively supported via Xcode, nothing additional besides the
rustup target should be needed.
",
        );
    } else if cfg!(target_os = "macos") {
        message.push_str(
            "
macOS on x86_64 cross tests to target x86_64-apple-ios, which requires the iOS
SDK to be installed. This should be included with Xcode automatically. If you
are using the Xcode command line tools, you'll need to install the full Xcode
app (from the Apple App Store), and switch to it with this command:

    sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer

Some cross-tests want to *run* the executables on the host. These tests will
be ignored if this is not possible. On macOS, this means you need an iOS
simulator installed to run these tests. To install a simulator, open Xcode, go
to preferences > Components, and download the latest iOS simulator.
",
        );
    } else if cfg!(target_os = "windows") {
        message.push_str(
            "
Windows cross tests target i686-pc-windows-msvc, which requires the ability
to build and run 32-bit targets. This should work automatically if you have
properly installed Visual Studio build tools.
",
        );
    } else {
        // The check at the top should prevent this.
        panic!("platform should have been skipped");
    }

    let rustup_available = Command::new("rustup").output().is_ok();
    if rustup_available {
        write!(
            message,
            "
Make sure that the appropriate `rustc` target is installed with rustup:

    rustup target add {}
",
            cross_target
        )
        .unwrap();
    } else {
        write!(
            message,
            "
rustup does not appear to be installed. Make sure that the appropriate
`rustc` target is installed for the target `{}`.
",
            cross_target
        )
        .unwrap();
    }

    // Show the actual error message.
    match run_cross_test() {
        Ok(_) => message.push_str("\nUh oh, second run succeeded?\n"),
        Err(err) => match err.downcast_ref::<ProcessError>() {
            Some(proc_err) => write!(message, "\nTest error: {}\n", proc_err).unwrap(),
            None => write!(message, "\nUnexpected non-process error: {}\n", err).unwrap(),
        },
    }

    panic!("{}", message);
}

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
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-ios"
    } else if cfg!(target_os = "linux") {
        "i686-unknown-linux-gnu"
    } else if cfg!(all(target_os = "windows", target_env = "msvc")) {
        "i686-pc-windows-msvc"
    } else if cfg!(all(target_os = "windows", target_env = "gnu")) {
        "i686-pc-windows-gnu"
    } else {
        panic!("This test should be gated on cross_compile::disabled.");
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

/// Whether or not the host can run cross-compiled executables.
pub fn can_run_on_host() -> bool {
    if disabled() {
        return false;
    }
    // macos is currently configured to cross compile to x86_64-apple-ios
    // which requires a simulator to run. Azure's CI image appears to have the
    // SDK installed, but are not configured to launch iOS images with a
    // simulator.
    if cfg!(target_os = "macos") {
        if CAN_RUN_ON_HOST.load(Ordering::SeqCst) {
            return true;
        } else {
            println!("Note: Cannot run on host, skipping.");
            return false;
        }
    } else {
        assert!(CAN_RUN_ON_HOST.load(Ordering::SeqCst));
        return true;
    }
}
