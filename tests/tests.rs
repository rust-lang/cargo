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

mod support;
macro_rules! test {
    ($name:ident $expr:expr) => (
        #[test]
        fn $name() {
            ::support::paths::setup();
            setup();
            $expr;
        }
    )
}

mod test_bad_config;
mod test_bad_manifest_path;
mod test_cargo;
mod test_cargo_bench;
mod test_cargo_build_auth;
mod test_cargo_build_lib;
mod test_cargo_clean;
mod test_cargo_compile;
mod test_cargo_compile_custom_build;
mod test_cargo_compile_git_deps;
mod test_cargo_compile_path_deps;
mod test_cargo_compile_plugins;
mod test_cargo_cross_compile;
mod test_cargo_doc;
mod test_cargo_features;
mod test_cargo_fetch;
mod test_cargo_freshness;
mod test_cargo_generate_lockfile;
mod test_cargo_init;
mod test_cargo_install;
mod test_cargo_metadata;
mod test_cargo_new;
mod test_cargo_package;
mod test_cargo_profiles;
mod test_cargo_publish;
mod test_cargo_read_manifest;
mod test_cargo_registry;
mod test_cargo_run;
mod test_cargo_rustc;
mod test_cargo_rustdoc;
mod test_cargo_search;
mod test_cargo_test;
mod test_cargo_tool_paths;
mod test_cargo_verify_project;
mod test_cargo_version;
mod test_shell;

thread_local!(static RUSTC: Rustc = Rustc::new("rustc").unwrap());

fn rustc_host() -> String {
    RUSTC.with(|r| r.host.clone())
}

fn is_nightly() -> bool {
    RUSTC.with(|r| {
        r.verbose_version.contains("-nightly") ||
            r.verbose_version.contains("-dev")
    })
}

fn can_panic() -> bool {
    RUSTC.with(|r| !(r.host.contains("msvc") && !r.host.contains("x86_64")))
}

fn process<T: AsRef<OsStr>>(t: T) -> cargo::util::ProcessBuilder {
    let mut p = cargo::util::process(t.as_ref());
    p.cwd(&support::paths::root())
     .env("HOME", &support::paths::home())
     .env_remove("CARGO_HOME")
     .env_remove("XDG_CONFIG_HOME")      // see #2345
     .env("GIT_CONFIG_NOSYSTEM", "1")    // keep trying to sandbox ourselves
     .env_remove("CARGO_TARGET_DIR")     // we assume 'target'
     .env_remove("MSYSTEM");             // assume cmd.exe everywhere on windows
    return p
}

fn cargo_process() -> cargo::util::ProcessBuilder {
    process(&support::cargo_dir().join("cargo"))
}

#[allow(deprecated)] // sleep_ms is now deprecated in favor of sleep()
fn sleep_ms(ms: u32) {
    std::thread::sleep_ms(ms);
}
