/*
# Introduction To `cargotest`

Cargo has a wide variety of integration tests that execute the `cargo` binary
and verify its behavior.  The `cargotest` module contains many helpers to make
this process easy.

The general form of a test involves creating a "project", running cargo, and
checking the result.  Projects are created with the `ProjectBuilder` where you
specify some files to create.  The general form looks like this:

```
let p = project()
    .file("Cargo.toml", &basic_bin_manifest("foo"))
    .file("src/main.rs", r#"fn main() { println!("hi!"); }"#)
    .build();
```

To run cargo, call the `cargo` method and use the `hamcrest` matchers to check
the output.

```
assert_that(
    p.cargo("run --bin foo"),
    execs()
        .with_status(0)
        .with_stderr(
            "\
[COMPILING] foo [..]
[FINISHED] [..]
[RUNNING] `target[/]debug[/]foo`
",
        )
        .with_stdout("hi!"),
);
```

The project creates a mini sandbox under the "cargo integration test"
directory with each test getting a separate directory such as
`/path/to/cargo/target/cit/t123/`.  Each project appears as a separate
directory.  There is also an empty `home` directory created that will be used
as a home directory instead of your normal home directory.

See `cargotest::support::lines_match` for an explanation of the string pattern
matching.

See the `hamcrest` module for other matchers like
`is_not(existing_file(path))`.  This is not the actual hamcrest library, but
instead a lightweight subset of matchers that are used in cargo tests.

Browse the `pub` functions in the `cargotest` module for a variety of other
helpful utilities.

## Testing Nightly Features

If you are testing a Cargo feature that only works on "nightly" cargo, then
you need to call `masquerade_as_nightly_cargo` on the process builder like
this:

```
p.cargo("build").masquerade_as_nightly_cargo()
```

If you are testing a feature that only works on *nightly rustc* (such as
benchmarks), then you should exit the test if it is not running with nightly
rust, like this:

```
if !is_nightly() {
    return;
}
```

## Platform-specific Notes

When checking output, be sure to use `[/]` when checking paths to
automatically support backslashes on Windows.

Be careful when executing binaries on Windows.  You should not rename, delete,
or overwrite a binary immediately after running it.  Under some conditions
Windows will fail with errors like "directory not empty" or "failed to remove"
or "access is denied".

*/

use std::ffi::OsStr;
use std::time::Duration;

use cargo::util::Rustc;
use cargo;
use std::path::{Path, PathBuf};

#[macro_use]
pub mod support;

pub mod install;

thread_local!(
pub static RUSTC: Rustc = Rustc::new(
    PathBuf::from("rustc"),
    None,
    Path::new("should be path to rustup rustc, but we don't care in tests"),
    None,
).unwrap()
);

/// The rustc host such as `x86_64-unknown-linux-gnu`.
pub fn rustc_host() -> String {
    RUSTC.with(|r| r.host.clone())
}

pub fn is_nightly() -> bool {
    RUSTC.with(|r| r.verbose_version.contains("-nightly") || r.verbose_version.contains("-dev"))
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

     // Force cargo to think it's on the stable channel for all tests, this
     // should hopefully not surprise us as we add cargo features over time and
     // cargo rides the trains.
     .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "stable")

     // For now disable incremental by default as support hasn't ridden to the
     // stable channel yet. Once incremental support hits the stable compiler we
     // can switch this to one and then fix the tests.
     .env("CARGO_INCREMENTAL", "0")

     // This env var can switch the git backend from libgit2 to git2-curl, which
     // can tweak error messages and cause some tests to fail, so let's forcibly
     // remove it.
     .env_remove("CARGO_HTTP_CHECK_REVOKE")

     .env_remove("__CARGO_DEFAULT_LIB_METADATA")
     .env_remove("RUSTC")
     .env_remove("RUSTDOC")
     .env_remove("RUSTC_WRAPPER")
     .env_remove("RUSTFLAGS")
     .env_remove("XDG_CONFIG_HOME")      // see #2345
     .env("GIT_CONFIG_NOSYSTEM", "1")    // keep trying to sandbox ourselves
     .env_remove("EMAIL")
     .env_remove("MFLAGS")
     .env_remove("MAKEFLAGS")
     .env_remove("CARGO_MAKEFLAGS")
     .env_remove("GIT_AUTHOR_NAME")
     .env_remove("GIT_AUTHOR_EMAIL")
     .env_remove("GIT_COMMITTER_NAME")
     .env_remove("GIT_COMMITTER_EMAIL")
     .env_remove("CARGO_TARGET_DIR")     // we assume 'target'
     .env_remove("MSYSTEM"); // assume cmd.exe everywhere on windows
    return p;
}

pub trait ChannelChanger: Sized {
    fn masquerade_as_nightly_cargo(&mut self) -> &mut Self;
}

impl ChannelChanger for cargo::util::ProcessBuilder {
    fn masquerade_as_nightly_cargo(&mut self) -> &mut Self {
        self.env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
    }
}

pub fn cargo_process() -> cargo::util::ProcessBuilder {
    process(&support::cargo_exe())
}

pub fn sleep_ms(ms: u64) {
    ::std::thread::sleep(Duration::from_millis(ms));
}
