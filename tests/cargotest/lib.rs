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

use cargo::util::Rustc;
use std::ffi::OsStr;
use std::time::Duration;
use std::path::PathBuf;
use std::env;

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
    _process(t.as_ref())
}

fn _process(t: &OsStr) -> cargo::util::ProcessBuilder {
    let mut p = cargo::util::process(t);
    p.cwd(&support::paths::root())
     .env_remove("CARGO_HOME")
     .env("HOME", support::paths::home())
     .env("CARGO_HOME", support::paths::home().join(".cargo"))
     .env_remove("RUSTC")
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
    // Note that this isn't needed on Windows as we assume the bindir (with
    // dlls) is in PATH.
    if cfg!(unix) {
        let var = if cfg!(target_os = "macos") {
            "DYLD_LIBRARY_PATH"
        } else {
            "LD_LIBRARY_PATH"
        };
        let rustc = RUSTC.with(|r| r.path.clone());
        let path = env::var_os("PATH").unwrap_or(Default::default());
        let rustc = env::split_paths(&path)
                        .map(|p| p.join(&rustc))
                        .find(|p| p.exists())
                        .unwrap();
        let mut libdir = rustc.clone();
        libdir.pop();
        libdir.pop();
        libdir.push("lib");
        let prev = env::var_os(&var).unwrap_or(Default::default());
        let mut paths = env::split_paths(&prev).collect::<Vec<_>>();
        println!("libdir: {:?}", libdir);
        if !paths.contains(&libdir) {
            paths.push(libdir);
            p.env(var, env::join_paths(&paths).unwrap());
        }
    }
    return p
}

pub fn cargo_process() -> cargo::util::ProcessBuilder {
    process(&support::cargo_exe())
}

pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}
