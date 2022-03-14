use std::{ffi::OsStr, path::Path};

/// The rustc-link-arg instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option](https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg)
/// to the compiler, but only when building supported targets (benchmarks,
/// binaries, cdylib crates, examples, and tests). Its usage is highly platform
/// specific. It is useful to set the shared library version or linker script.
pub fn rustc_link_arg(flag: &str) {
    println!("cargo:rustc-link-arg={flag}");
}

/// The `rustc-link-arg-bin` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option](https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg)
/// to the compiler, but only when building the binary target with name `BIN`.
/// Its usage is highly platform specific. It is useful to set a linker script
/// or other linker options.
pub fn rustc_link_arg_bin(bin: &str, flag: &str) {
    println!("cargo:rustc-link-arg-bin={bin}={flag}");
}

/// The `rustc-link-arg-bins` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option](https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg)
/// to the compiler, but only when building a binary target. Its usage is
/// highly platform specific. It is useful to set a linker script or other
/// linker options.
pub fn rustc_link_arg_bins(flag: &str) {
    println!("cargo:rustc-link-arg-bins={flag}");
}

/// The `rustc-link-lib` instruction tells Cargo to link the given library
/// using the compiler's [`-l` flag]. This is typically used to link a native
/// library using [`FFI`].
///
/// The `-l` flag is only passed to the library target of the package, unless
/// there is no library target, in which case it is passed to all targets. This
/// is done because all other targets have an implicit dependency on the
/// library target, and the given library to link should only be included once.
/// This means that if a package has both a library and a binary target, the
/// _library_ has access to the symbols from the given lib, and the binary
/// should access them through the library target's public API.
///
/// The optional `KIND` may be one of `dylib`, `static`, or `framework`.
/// See the [rustc book] for more detail.
///
/// [`-l` flag]: https://doc.rust-lang.org/rustc/command-line-arguments.html#option-l-link-lib
/// [rustc book]: https://doc.rust-lang.org/rustc/command-line-arguments.html#option-l-link-lib
pub fn rustc_link_lib(name: &str) {
    println!("cargo:rustc-link-lib={name}");
}

/// See [`rustc_link_lib`].
pub fn rustc_link_lib_kind(kind: &str, name: &str) {
    println!("cargo:rustc-link-lib={kind}={name}");
}

/// The `rustc-link-search` instruction tells Cargo to pass the [`-L flag`] to
/// the compiler to add a directory to the library search path.
///
/// The optional `KIND` may be one of `dependency`, `crate`, `native`,
/// `framework`, or `all`. See the [rustc book] for more detail.
///
/// These paths are also added to the
/// [dynamic library search path environment variable] if they are within the
/// `OUT_DIR`. Depending on this behavior is discouraged since this makes it
/// difficult to use the resulting binary. In general, it is best to avoid
/// creating dynamic libraries in a build script (using existing system
/// libraries is fine).
///
/// [`-L flag`]: https://doc.rust-lang.org/rustc/command-line-arguments.html#option-l-search-path
/// [rustc book]: https://doc.rust-lang.org/rustc/command-line-arguments.html#option-l-search-path
/// [dynamic library search path environment variable]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#dynamic-library-paths
pub fn rustc_link_search(path: impl AsRef<Path>) {
    let path = path
        .as_ref()
        .as_os_str()
        .to_str()
        .expect("cannot print non-UTF8 path");
    println!("cargo:rustc-link-search={path}");
}

/// See [`rustc_link_search`].
pub fn rustc_link_search_kind(kind: &str, path: impl AsRef<Path>) {
    let path = path
        .as_ref()
        .as_os_str()
        .to_str()
        .expect("cannot print non-UTF8 path");
    println!("cargo:rustc-link-search={kind}={path}");
}

/// The `rustc-flags` instruction tells Cargo to pass the given space-separated
/// flags to the compiler. This only allows the `-l` and `-L` flags, and is
/// equivalent to using `rustc-link-lib` and `rustc-link-search`.
pub fn rustc_flags(flags: &str) {
    println!("cargo:rustc-flags={flags}");
}

/// The `rustc-cfg` instruction tells Cargo to pass the given value to the
/// [`--cfg` flag](https://doc.rust-lang.org/rustc/command-line-arguments.html#option-cfg)
/// to the compiler. This may be used for compile-time detection of features to
/// enable [conditional compilation](https://doc.rust-lang.org/reference/conditional-compilation.html).
///
/// Note that this does not affect Cargo's dependency resolution. This cannot
/// be used to enable an optional dependency, or enable other Cargo features.
///
/// Be aware that Cargo features use the form `feature="foo"`. `cfg` values
/// passed with this flag are not restricted to that form, and may provide just
/// a single identifier, or any arbitrary key/value pair. For example, emitting
/// `cargo:rustc-cfg=abc` will then allow code to use `#[cfg(abc)]` (note the
/// lack of `feature=`). Or an arbitrary key/value pair may be used with an `=`
/// symbol like `cargo:rustc-cfg=my_component="foo"`. The key should be a Rust
/// identifier, the value should be a string.
pub fn rustc_cfg(key: &str) {
    println!("cargo:rustc-cfg={key}")
}

/// See [`rustc_cfg`].
pub fn rustc_cfg_value(key: &str, value: &str) {
    println!("cargo:rustc-cfg={key}={value}");
}

/// The `rustc-env` instruction tells Cargo to set the given environment
/// variable when compiling the package. The value can be then retrieved by the
/// [`env! macro`](env!) in the compiled crate. This is useful for embedding
/// additional metadata in crate's code, such as the hash of git HEAD or the
/// unique identifier of a continuous integration server.
pub fn rustc_env(var: &str, value: &str) {
    println!("cargo:rustc-env={var}={value}");
}

/// The `rustc-cdylib-link-arg` instruction tells Cargo to pass the
/// [`-C link-arg=FLAG` option](https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg)
/// to the compiler, but only when building a cdylib library target. Its usage
/// is highly platform specific. It is useful to set the shared library version
/// or the runtime-path.
pub fn rustc_cdylib_link_arg(flag: &str) {
    println!("cargo:rustc-cdylib-link-arg={flag}");
}

/// The `warning` instruction tells Cargo to display a warning after the build
/// script has finished running. Warnings are only shown for path dependencies
/// (that is, those you're working on locally), so for example warnings printed
/// out in crates.io crates are not emitted by default. The `-vv` "very verbose"
/// flag may be used to have Cargo display warnings for all crates.
pub fn warning(message: &str) {
    println!("cargo:warning={message}");
}

/// The `rerun-if-changed` instruction tells Cargo to re-run the build script
/// if the file at the given path has changed. Currently, Cargo only uses the
/// filesystem last-modified "mtime" timestamp to determine if the file has
/// changed. It compares against an internal cached timestamp of when the build
/// script last ran.
///
/// If the path points to a directory, it will scan the entire directory for
/// any modifications.
///
/// If the build script inherently does not need to re-run under any
/// circumstance, then emitting `cargo:rerun-if-changed=build.rs` is a simple
/// way to prevent it from being re-run (otherwise, the default if no
/// `rerun-if` instructions are emitted is to scan the entire package
/// directory for changes). Cargo automatically handles whether or not the
/// script itself needs to be recompiled, and of course the script will be
/// re-run after it has been recompiled. Otherwise, specifying build.rs is
/// redundant and unnecessary.
pub fn rerun_if_changed(path: impl AsRef<Path>) {
    let path = path
        .as_ref()
        .as_os_str()
        .to_str()
        .expect("cannot print non-UTF8 path");
    println!("cargo:rerun-if-changed={path}");
}

/// The `rerun-if-env-changed` instruction tells Cargo to re-run the build
/// script if the value of an environment variable of the given name has
/// changed.
///
/// Note that the environment variables here are intended for global
/// environment variables like `CC` and such, it is not necessary to use this
/// for environment variables like `TARGET` that Cargo sets.
pub fn rerun_if_env_changed(name: impl AsRef<OsStr>) {
    let name = name
        .as_ref()
        .to_str()
        .expect("cannot print non-UTF8 env key");
    println!("cargo:rerun-if-env-changed={name}");
}
