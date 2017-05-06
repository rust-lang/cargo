#![deny(warnings)]

extern crate bufstream;
extern crate cargo;
extern crate filetime;
extern crate flate2;
extern crate git2;
extern crate hamcrest;
extern crate libc;
extern crate rustc_serialize;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate tar;
extern crate tempdir;
extern crate term;
extern crate url;
#[cfg(windows)] extern crate kernel32;
#[cfg(windows)] extern crate winapi;

use std::ffi::OsStr;
use std::time::Duration;
use std::path::{Path, PathBuf};

use cargo::util::Rustc;
use cargo::util::paths;

pub mod support;
pub mod install;

thread_local!(pub static RUSTC: Rustc = Rustc::new(PathBuf::from("rustc"), None).unwrap());

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
    _process(t.as_ref())
}

fn _process(t: &OsStr) -> cargo::util::ProcessBuilder {
    let mut p = cargo::util::process(t);
    p.cwd(&support::paths::root())
     .env_remove("CARGO_HOME")
     .env("HOME", support::paths::home())
     .env("CARGO_HOME", support::paths::home().join(".cargo"))
     .env("__CARGO_TEST_ROOT", support::paths::root())
     .env_remove("__CARGO_DEFAULT_LIB_METADATA")
     .env_remove("RUSTC")
     .env_remove("RUSTDOC")
     .env_remove("RUSTC_WRAPPER")
     .env_remove("RUSTFLAGS")
     .env_remove("CARGO_INCREMENTAL")
     .env_remove("XDG_CONFIG_HOME")      // see #2345
     .env("GIT_CONFIG_NOSYSTEM", "1")    // keep trying to sandbox ourselves
     .env_remove("EMAIL")
     .env_remove("GIT_AUTHOR_NAME")
     .env_remove("GIT_AUTHOR_EMAIL")
     .env_remove("GIT_COMMITTER_NAME")
     .env_remove("GIT_COMMITTER_EMAIL")
     .env_remove("CARGO_TARGET_DIR")     // we assume 'target'
     .env_remove("MSYSTEM");             // assume cmd.exe everywhere on windows

    // We'll need dynamic libraries at some point in this test suite, so ensure
    // that the rustc libdir is somewhere in LD_LIBRARY_PATH as appropriate.
    let mut rustc = RUSTC.with(|r| r.process());
    let output = rustc.arg("--print").arg("sysroot").exec_with_output().unwrap();
    let libdir = String::from_utf8(output.stdout).unwrap();
    let libdir = Path::new(libdir.trim());
    let libdir = if cfg!(windows) {
        libdir.join("bin")
    } else {
        libdir.join("lib")
    };
    let mut paths = paths::dylib_path();
    println!("libdir: {:?}", libdir);
    if !paths.contains(&libdir) {
        paths.push(libdir);
        p.env(paths::dylib_path_envvar(),
              paths::join_paths(&paths, paths::dylib_path_envvar()).unwrap());
    }
    return p
}

pub fn cargo_process() -> cargo::util::ProcessBuilder {
    process(&support::cargo_exe())
}

pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}
