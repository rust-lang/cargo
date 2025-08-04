//! Outputs from the build script to the build system.
//!
//! This crate assumes that stdout is at a new line whenever an output directive
//! is called. Printing to stdout without a terminating newline (i.e. not using
//! [`println!`]) may lead to surprising behavior.
//!
//! Reference: <https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script>

use std::ffi::OsStr;
use std::path::Path;
use std::{fmt::Display, fmt::Write as _};

use crate::ident::{is_ascii_ident, is_ident};

fn emit(directive: &str, value: impl Display) {
    println!("cargo::{directive}={value}");
}

/// The `rerun-if-changed` instruction tells Cargo to re-run the build script if the
/// file at the given path has changed.
///
/// Currently, Cargo only uses the filesystem
/// last-modified “mtime” timestamp to determine if the file has changed. It
/// compares against an internal cached timestamp of when the build script last ran.
///
/// If the path points to a directory, it will scan the entire directory for any
/// modifications.
///
/// If the build script inherently does not need to re-run under any circumstance,
/// then calling `rerun_if_changed("build.rs")` is a simple way to prevent it from
/// being re-run (otherwise, the default if no `rerun-if` instructions are emitted
/// is to scan the entire package directory for changes). Cargo automatically
/// handles whether or not the script itself needs to be recompiled, and of course
/// the script will be re-run after it has been recompiled. Otherwise, specifying
/// `build.rs` is redundant and unnecessary.
#[track_caller]
pub fn rerun_if_changed(path: impl AsRef<Path>) {
    let Some(path) = path.as_ref().to_str() else {
        panic!("cannot emit rerun-if-changed: path is not UTF-8");
    };
    if path.contains('\n') {
        panic!("cannot emit rerun-if-changed: path contains newline");
    }
    emit("rerun-if-changed", path);
}

/// The `rerun-if-env-changed` instruction tells Cargo to re-run the build script
/// if the value of an environment variable of the given name has changed.
///
/// Note that the environment variables here are intended for global environment
/// variables like `CC` and such, it is not possible to use this for environment
/// variables like `TARGET` that [Cargo sets for build scripts][build-env]. The
/// environment variables in use are those received by cargo invocations, not
/// those received by the executable of the build script.
///
/// [build-env]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
#[track_caller]
pub fn rerun_if_env_changed(key: impl AsRef<OsStr>) {
    let Some(key) = key.as_ref().to_str() else {
        panic!("cannot emit rerun-if-env-changed: key is not UTF-8");
    };
    if key.contains('\n') {
        panic!("cannot emit rerun-if-env-changed: key contains newline");
    }
    emit("rerun-if-env-changed", key);
}

/// The `rustc-link-arg` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option][link-arg] to the compiler, but only when building
/// supported targets (benchmarks, binaries, cdylib crates, examples, and tests).
///
/// Its usage is highly platform specific. It is useful to set the shared library
/// version or linker script.
///
/// [link-arg]: https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg
#[track_caller]
pub fn rustc_link_arg(flag: &str) {
    if flag.contains([' ', '\n']) {
        panic!("cannot emit rustc-link-arg: invalid flag {flag:?}");
    }
    emit("rustc-link-arg", flag);
}

/// The `rustc-link-arg-bin` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option][link-arg] to the compiler, but only when building
/// the binary target with name `BIN`. Its usage is highly platform specific.
///
/// It
/// is useful to set a linker script or other linker options.
///
/// [link-arg]: https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg
#[track_caller]
pub fn rustc_link_arg_bin(bin: &str, flag: &str) {
    if !is_ident(bin) {
        panic!("cannot emit rustc-link-arg-bin: invalid bin name {bin:?}");
    }
    if flag.contains([' ', '\n']) {
        panic!("cannot emit rustc-link-arg-bin: invalid flag {flag:?}");
    }
    emit("rustc-link-arg-bin", format_args!("{bin}={flag}"));
}

/// The `rustc-link-arg-bins` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option][link-arg] to the compiler, but only when building
/// the binary target.
///
/// Its usage is highly platform specific. It is useful to set
/// a linker script or other linker options.
///
/// [link-arg]: https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg
#[track_caller]
pub fn rustc_link_arg_bins(flag: &str) {
    if flag.contains([' ', '\n']) {
        panic!("cannot emit rustc-link-arg-bins: invalid flag {flag:?}");
    }
    emit("rustc-link-arg-bins", flag);
}

/// The `rustc-link-arg-tests` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option][link-arg] to the compiler, but only when building
/// a tests target.
///
/// [link-arg]: https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg
#[track_caller]
pub fn rustc_link_arg_tests(flag: &str) {
    if flag.contains([' ', '\n']) {
        panic!("cannot emit rustc-link-arg-tests: invalid flag {flag:?}");
    }
    emit("rustc-link-arg-tests", flag);
}

/// The `rustc-link-arg-examples` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option][link-arg] to the compiler, but only when building
/// an examples target.
///
/// [link-arg]: https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg
#[track_caller]
pub fn rustc_link_arg_examples(flag: &str) {
    if flag.contains([' ', '\n']) {
        panic!("cannot emit rustc-link-arg-examples: invalid flag {flag:?}");
    }
    emit("rustc-link-arg-examples", flag);
}

/// The `rustc-link-arg-benches` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option][link-arg] to the compiler, but only when building
/// a benchmark target.
///
/// [link-arg]: https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg
#[track_caller]
pub fn rustc_link_arg_benches(flag: &str) {
    if flag.contains([' ', '\n']) {
        panic!("cannot emit rustc-link-arg-benches: invalid flag {flag:?}");
    }
    emit("rustc-link-arg-benches", flag);
}

/// The `rustc-link-lib` instruction tells Cargo to link the given library using
/// the compiler’s [`-l` flag][-l].
///
/// This is typically used to link a native library
/// using [FFI].
///
/// The `LIB` string is passed directly to rustc, so it supports any syntax that
/// `-l` does. Currently the full supported syntax for `LIB` is
/// `[KIND[:MODIFIERS]=]NAME[:RENAME]`.
///
/// The `-l` flag is only passed to the library target of the package, unless there
/// is no library target, in which case it is passed to all targets. This is done
/// because all other targets have an implicit dependency on the library target,
/// and the given library to link should only be included once. This means that
/// if a package has both a library and a binary target, the library has access
/// to the symbols from the given lib, and the binary should access them through
/// the library target’s public API.
///
/// The optional `KIND` may be one of `dylib`, `static`, or `framework`. See the
/// [rustc book][-l] for more detail.
///
/// [-l]: https://doc.rust-lang.org/stable/rustc/command-line-arguments.html#option-l-link-lib
/// [FFI]: https://doc.rust-lang.org/stable/nomicon/ffi.html
#[track_caller]
pub fn rustc_link_lib(lib: &str) {
    if lib.contains([' ', '\n']) {
        panic!("cannot emit rustc-link-lib: invalid lib {lib:?}");
    }
    emit("rustc-link-lib", lib);
}

/// Like [`rustc_link_lib`], but with `KIND[:MODIFIERS]` specified separately.
#[track_caller]
pub fn rustc_link_lib_kind(kind: &str, lib: &str) {
    if kind.contains(['=', ' ', '\n']) {
        panic!("cannot emit rustc-link-lib: invalid kind {kind:?}");
    }
    if lib.contains([' ', '\n']) {
        panic!("cannot emit rustc-link-lib: invalid lib {lib:?}");
    }
    emit("rustc-link-lib", format_args!("{kind}={lib}"));
}

/// The `rustc-link-search` instruction tells Cargo to pass the [`-L` flag] to the
/// compiler to add a directory to the library search path.
///
/// The optional `KIND` may be one of `dependency`, `crate`, `native`, `framework`,
/// or `all`. See the [rustc book][-L] for more detail.
///
/// These paths are also added to the
/// [dynamic library search path environment variable][search-path] if they are
/// within the `OUT_DIR`. Depending on this behavior is discouraged since this
/// makes it difficult to use the resulting binary. In general, it is best to
/// avoid creating dynamic libraries in a build script (using existing system
/// libraries is fine).
///
/// [-L]: https://doc.rust-lang.org/stable/rustc/command-line-arguments.html#option-l-search-path
/// [search-path]: https://doc.rust-lang.org/stable/cargo/reference/environment-variables.html#dynamic-library-paths
#[track_caller]
pub fn rustc_link_search(path: impl AsRef<Path>) {
    let Some(path) = path.as_ref().to_str() else {
        panic!("cannot emit rustc-link-search: path is not UTF-8");
    };
    if path.contains('\n') {
        panic!("cannot emit rustc-link-search: path contains newline");
    }
    emit("rustc-link-search", path);
}

/// Like [`rustc_link_search`], but with KIND specified separately.
#[track_caller]
pub fn rustc_link_search_kind(kind: &str, path: impl AsRef<Path>) {
    if kind.contains(['=', '\n']) {
        panic!("cannot emit rustc-link-search: invalid kind {kind:?}");
    }
    let Some(path) = path.as_ref().to_str() else {
        panic!("cannot emit rustc-link-search: path is not UTF-8");
    };
    if path.contains('\n') {
        panic!("cannot emit rustc-link-search: path contains newline");
    }
    emit("rustc-link-search", format_args!("{kind}={path}"));
}

/// The `rustc-flags` instruction tells Cargo to pass the given space-separated
/// flags to the compiler.
///
/// This only allows the `-l` and `-L` flags, and is
/// equivalent to using [`rustc_link_lib`] and [`rustc_link_search`].
#[track_caller]
pub fn rustc_flags(flags: &str) {
    if flags.contains('\n') {
        panic!("cannot emit rustc-flags: invalid flags");
    }
    emit("rustc-flags", flags);
}

/// The `rustc-cfg` instruction tells Cargo to pass the given value to the
/// [`--cfg` flag][cfg] to the compiler.
///
/// This may be used for compile-time
/// detection of features to enable conditional compilation.
///
/// Note that this does not affect Cargo’s dependency resolution. This cannot
/// be used to enable an optional dependency, or enable other Cargo features.
///
/// Be aware that [Cargo features] use the form `feature="foo"`. `cfg` values
/// passed with this flag are not restricted to that form, and may provide just
/// a single identifier, or any arbitrary key/value pair. For example, emitting
/// `rustc_cfg("abc")` will then allow code to use `#[cfg(abc)]` (note the lack
/// of `feature=`). Or an arbitrary key/value pair may be used with an `=` symbol
/// like `rustc_cfg(r#"my_component="foo""#)`. The key should be a Rust identifier,
/// the value should be a string.
///
/// [cfg]: https://doc.rust-lang.org/rustc/command-line-arguments.html#option-cfg
/// [Cargo features]: https://doc.rust-lang.org/cargo/reference/features.html
#[track_caller]
pub fn rustc_cfg(key: &str) {
    if !is_ident(key) {
        panic!("cannot emit rustc-cfg: invalid key {key:?}");
    }
    emit("rustc-cfg", key);
}

/// Like [`rustc_cfg`], but with the value specified separately.
///
/// To replace the
/// less convenient `rustc_cfg(r#"my_component="foo""#)`, you can instead use
/// `rustc_cfg_value("my_component", "foo")`.
#[track_caller]
pub fn rustc_cfg_value(key: &str, value: &str) {
    if !is_ident(key) {
        panic!("cannot emit rustc-cfg-value: invalid key");
    }
    let value = value.escape_default();
    emit("rustc-cfg", format_args!("{key}=\"{value}\""));
}

/// Add to the list of expected config names that is used when checking the
/// *reachable* cfg expressions with the [`unexpected_cfgs`] lint.
///
/// This form is for keys without an expected value, such as `cfg(name)`.
///
/// It is recommended to group the `rustc_check_cfg` and `rustc_cfg` calls as
/// closely as possible in order to avoid typos, missing check_cfg, stale cfgs,
/// and other mistakes.
///
/// [`unexpected_cfgs`]: https://doc.rust-lang.org/rustc/lints/listing/warn-by-default.html#unexpected-cfgs
#[doc = respected_msrv!("1.80")]
#[track_caller]
pub fn rustc_check_cfgs(keys: &[&str]) {
    if keys.is_empty() {
        return;
    }
    for key in keys {
        if !is_ident(key) {
            panic!("cannot emit rustc-check-cfg: invalid key {key:?}");
        }
    }

    let mut directive = keys[0].to_string();
    for key in &keys[1..] {
        write!(directive, ", {key}").expect("writing to string should be infallible");
    }
    emit("rustc-check-cfg", format_args!("cfg({directive})"));
}

/// Add to the list of expected config names that is used when checking the
/// *reachable* cfg expressions with the [`unexpected_cfgs`] lint.
///
/// This form is for keys with expected values, such as `cfg(name = "value")`.
///
/// It is recommended to group the `rustc_check_cfg` and `rustc_cfg` calls as
/// closely as possible in order to avoid typos, missing check_cfg, stale cfgs,
/// and other mistakes.
///
/// [`unexpected_cfgs`]: https://doc.rust-lang.org/rustc/lints/listing/warn-by-default.html#unexpected-cfgs
#[doc = respected_msrv!("1.80")]
#[track_caller]
pub fn rustc_check_cfg_values(key: &str, values: &[&str]) {
    if !is_ident(key) {
        panic!("cannot emit rustc-check-cfg: invalid key {key:?}");
    }
    if values.is_empty() {
        rustc_check_cfgs(&[key]);
        return;
    }

    let mut directive = format!("\"{}\"", values[0].escape_default());
    for value in &values[1..] {
        write!(directive, ", \"{}\"", value.escape_default())
            .expect("writing to string should be infallible");
    }
    emit(
        "rustc-check-cfg",
        format_args!("cfg({key}, values({directive}))"),
    );
}

/// The `rustc-env` instruction tells Cargo to set the given environment variable
/// when compiling the package.
///
/// The value can be then retrieved by the
/// [`env!` macro][env!] in the compiled crate. This is useful for embedding
/// additional metadata in crate’s code, such as the hash of git HEAD or the
/// unique identifier of a continuous integration server.
///
/// See also the [environment variables automatically included by Cargo][cargo-env].
///
/// [cargo-env]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
#[track_caller]
pub fn rustc_env(key: &str, value: &str) {
    if key.contains(['=', '\n']) {
        panic!("cannot emit rustc-env: invalid key {key:?}");
    }
    if value.contains('\n') {
        panic!("cannot emit rustc-env: invalid value {value:?}");
    }
    emit("rustc-env", format_args!("{key}={value}"));
}

/// The `rustc-cdylib-link-arg` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option][link-arg] to the compiler, but only when building
/// a `cdylib` library target.
///
/// Its usage is highly platform specific. It is useful
/// to set the shared library version or the runtime-path.
///
/// [link-arg]: https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg
#[track_caller]
pub fn rustc_cdylib_link_arg(flag: &str) {
    if flag.contains('\n') {
        panic!("cannot emit rustc-cdylib-link-arg: invalid flag {flag:?}");
    }
    emit("rustc-cdylib-link-arg", flag);
}

/// The `warning` instruction tells Cargo to display a warning after the build
/// script has finished running.
///
/// Warnings are only shown for path dependencies
/// (that is, those you’re working on locally), so for example warnings printed
/// out in [crates.io] crates are not emitted by default. The `-vv` “very verbose”
/// flag may be used to have Cargo display warnings for all crates.
///
/// [crates.io]: https://crates.io/
#[track_caller]
pub fn warning(message: &str) {
    if message.contains('\n') {
        panic!("cannot emit warning: message contains newline");
    }
    emit("warning", message);
}

/// The `error` instruction tells Cargo to display an error after the build script has finished
/// running, and then fail the build.
///
/// <div class="warning">
///
/// Build script libraries should carefully consider if they want to use [`error`] versus
/// returning a `Result`. It may be better to return a `Result`, and allow the caller to decide if the
/// error is fatal or not. The caller can then decide whether or not to display the `Err` variant
/// using [`error`].
///
/// </div>
#[doc = respected_msrv!("1.84")]
#[track_caller]
pub fn error(message: &str) {
    if message.contains('\n') {
        panic!("cannot emit error: message contains newline");
    }
    emit("error", message);
}

/// Metadata, used by `links` scripts.
#[track_caller]
pub fn metadata(key: &str, val: &str) {
    if !is_ascii_ident(key) {
        panic!("cannot emit metadata: invalid key {key:?}");
    }
    if val.contains('\n') {
        panic!("cannot emit metadata: invalid value {val:?}");
    }

    emit("metadata", format_args!("{key}={val}"));
}
