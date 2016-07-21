#![deny(warnings)]

extern crate bufstream;
extern crate cargo;
extern crate filetime;
extern crate flate2;
extern crate git2;
extern crate hamcrest;
extern crate libc;
extern crate rustc_serialize;
extern crate tar;
extern crate tempdir;
extern crate term;
extern crate url;
#[cfg(windows)] extern crate kernel32;
#[cfg(windows)] extern crate winapi;

#[macro_use]
extern crate log;

use cargo::util::Rustc;
use std::ffi::OsStr;
use std::time::Duration;
use std::path::PathBuf;

pub mod support;
pub mod install;

thread_local!(pub static RUSTC: Rustc = Rustc::new(PathBuf::from("rustc")).unwrap());

pub fn rustc_host() -> String {
    RUSTC.with(|r| r.host.clone())
}

pub fn is_nightly() -> bool {
    RUSTC.with(|r| {
        r.verbose_version.contains("-nightly") ||
            r.verbose_version.contains("-dev")
    })
}

pub fn process<T: AsRef<OsStr>>(t: T) -> cargo::util::ProcessBuilder {
    let mut p = cargo::util::process(t.as_ref());
    p.cwd(&support::paths::root())
     .env_remove("CARGO_HOME")
     .env("HOME", support::paths::home())
     .env("CARGO_HOME", support::paths::home().join(".cargo"))
     .env_remove("RUSTC")
     .env_remove("RUSTFLAGS")
     .env_remove("XDG_CONFIG_HOME")      // see #2345
     .env("GIT_CONFIG_NOSYSTEM", "1")    // keep trying to sandbox ourselves
     .env_remove("CARGO_TARGET_DIR")     // we assume 'target'
     .env_remove("MSYSTEM");             // assume cmd.exe everywhere on windows
    return p
}

pub fn cargo_process() -> cargo::util::ProcessBuilder {
    process(&support::cargo_dir().join("cargo"))
}

pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}
