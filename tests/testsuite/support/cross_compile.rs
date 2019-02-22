use std::env;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Once, ONCE_INIT};

use crate::support::{basic_bin_manifest, main_file, project};

pub fn disabled() -> bool {
    // First, disable if `./configure` requested so.
    match env::var("CFG_DISABLE_CROSS_TESTS") {
        Ok(ref s) if *s == "1" => return true,
        _ => {}
    }

    // Right now, the Windows bots cannot cross compile due to the Mingw setup,
    // so we disable ourselves on all but macOS/Linux setups where the rustc
    // install script ensures we have both architectures.
    if !(cfg!(target_os = "macos") || cfg!(target_os = "linux") || cfg!(target_env = "msvc")) {
        return true;
    }

    // It's not particularly common to have a cross-compilation setup, so
    // try to detect that before we fail a bunch of tests through no fault
    // of the user.
    static CAN_RUN_CROSS_TESTS: AtomicBool = AtomicBool::new(false);
    static CHECK: Once = ONCE_INIT;

    let cross_target = alternate();

    CHECK.call_once(|| {
        let p = project()
            .at("cross_test")
            .file("Cargo.toml", &basic_bin_manifest("cross_test"))
            .file("src/cross_test.rs", &main_file(r#""testing!""#, &[]))
            .build();

        let result = p
            .cargo("build --target")
            .arg(&cross_target)
            .exec_with_output();

        if result.is_ok() {
            CAN_RUN_CROSS_TESTS.store(true, Ordering::SeqCst);
        }
    });

    if CAN_RUN_CROSS_TESTS.load(Ordering::SeqCst) {
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
    let rustup_available = Command::new("rustup").output().is_ok();

    let linux_help = if cfg!(target_os = "linux") {
        "

You may need to install runtime libraries for your Linux distribution as well."
            .to_string()
    } else {
        "".to_string()
    };

    let rustup_help = if rustup_available {
        format!(
            "

Alternatively, you can install the necessary libraries for cross-compilation with

    rustup target add {}{}",
            cross_target, linux_help
        )
    } else {
        "".to_string()
    };

    panic!(
        "Cannot cross compile to {}.

This failure can be safely ignored. If you would prefer to not see this
failure, you can set the environment variable CFG_DISABLE_CROSS_TESTS to \"1\".{}
",
        cross_target, rustup_help
    );
}

pub fn alternate() -> String {
    let platform = match env::consts::OS {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        _ => unreachable!(),
    };
    let arch = match env::consts::ARCH {
        "x86" => "x86_64",
        "x86_64" => "i686",
        _ => unreachable!(),
    };
    format!("{}-{}", arch, platform)
}

pub fn alternate_arch() -> &'static str {
    match env::consts::ARCH {
        "x86" => "x86_64",
        "x86_64" => "x86",
        _ => unreachable!(),
    }
}

pub fn host() -> String {
    let platform = match env::consts::OS {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        _ => unreachable!(),
    };
    let arch = match env::consts::ARCH {
        "x86" => "i686",
        "x86_64" => "x86_64",
        _ => unreachable!(),
    };
    format!("{}-{}", arch, platform)
}
