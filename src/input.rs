//! Inputs from the build system to the build script.
//!
//! This crate does not do any caching or interpreting of the values provided by
//! Cargo beyond the communication protocol itself. It is up to the build script
//! to interpret the string values and decide what to do with them.
//!
//! Reference: <https://doc.rust-lang.org/stable/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts>

use std::{
    env,
    fmt::Display,
    path::PathBuf,
    str::{self, FromStr},
};

macro_rules! missing {
    ($key:ident) => {
        panic!("cargo environment variable `{}` is missing", $key)
    };
}

macro_rules! invalid {
    ($key:ident, $err:expr) => {
        panic!("cargo environment variable `{}` is invalid: {}", $key, $err)
    };
}

#[track_caller]
fn get_bool(key: &str) -> bool {
    env::var_os(key).is_some()
}

#[track_caller]
fn get_opt_path(key: &str) -> Option<PathBuf> {
    let var = env::var_os(key)?;
    Some(PathBuf::from(var))
}

#[track_caller]
fn get_path(key: &str) -> PathBuf {
    get_opt_path(key).unwrap_or_else(|| missing!(key))
}

#[track_caller]
fn get_opt_str(key: &str) -> Option<String> {
    let var = env::var_os(key)?;
    match str::from_utf8(var.as_encoded_bytes()) {
        Ok(s) => Some(s.to_owned()),
        Err(err) => invalid!(key, err),
    }
}

#[track_caller]
fn get_str(key: &str) -> String {
    get_opt_str(key).unwrap_or_else(|| missing!(key))
}

#[track_caller]
fn get_num<T: FromStr>(key: &str) -> T
where
    T::Err: Display,
{
    let val = get_str(key);
    match val.parse() {
        Ok(num) => num,
        Err(err) => invalid!(key, err),
    }
}

#[track_caller]
fn get_opt_cfg(cfg: &str) -> (String, Option<Vec<String>>) {
    let cfg = cfg.to_uppercase().replace('-', "_");
    let key = format!("CARGO_CFG_{cfg}");
    let Some(var) = env::var_os(&key) else {
        return (key, None);
    };
    let val = str::from_utf8(var.as_encoded_bytes()).unwrap_or_else(|err| invalid!(key, err));
    (key, Some(val.split(',').map(str::to_owned).collect()))
}

#[track_caller]
fn get_cfg(cfg: &str) -> Vec<String> {
    let (key, val) = get_opt_cfg(cfg);
    val.unwrap_or_else(|| missing!(key))
}

// docs last updated to match release 1.77.2 reference

/// Path to the `cargo` binary performing the build.
pub fn cargo() -> PathBuf {
    get_path("CARGO")
}

/// The directory containing the manifest for the package being built (the package
/// containing the build script). Also note that this is the value of the current
/// working directory of the build script when it starts.
pub fn cargo_manifest_dir() -> PathBuf {
    get_path("CARGO_MANIFEST_DIR")
}

/// Contains parameters needed for Cargo’s [jobserver] implementation to parallelize
/// subprocesses. Rustc or cargo invocations from build.rs can already read
/// `CARGO_MAKEFLAGS`, but GNU Make requires the flags to be specified either
/// directly as arguments, or through the `MAKEFLAGS` environment variable.
/// Currently Cargo doesn’t set the `MAKEFLAGS` variable, but it’s free for build
/// scripts invoking GNU Make to set it to the contents of `CARGO_MAKEFLAGS`.
///
/// [jobserver]: https://www.gnu.org/software/make/manual/html_node/Job-Slots.html
pub fn cargo_manifest_links() -> Option<String> {
    get_opt_str("CARGO_MANIFEST_LINKS")
}

/// For each activated feature of the package being built, this will be `true`.
pub fn cargo_feature(name: &str) -> bool {
    let name = name.to_uppercase().replace('-', "_");
    let key = format!("CARGO_FEATURE_{name}");
    get_bool(&key)
}

/// For each [configuration option] of the package being built, this will contain
/// the value of the configuration. This includes values built-in to the compiler
/// (which can be seen with `rustc --print=cfg`) and values set by build scripts
/// and extra flags passed to rustc (such as those defined in `RUSTFLAGS`).
///
/// [configuration option]: https://doc.rust-lang.org/stable/reference/conditional-compilation.html
pub fn cargo_cfg(cfg: &str) -> Option<Vec<String>> {
    let (_, val) = get_opt_cfg(cfg);
    val
}

/// Set on [unix-like platforms](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#unix-and-windows).
pub fn cargo_cfg_unix() -> bool {
    get_bool("CARGO_CFG_UNIX")
}

/// Set on [windows-like platforms](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#unix-and-windows).
pub fn cargo_cfg_windows() -> bool {
    get_bool("CARGO_CFG_WINDOWS")
}

/// The [target family](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_family).
pub fn cargo_target_family() -> Vec<String> {
    get_cfg("target_family")
}

/// The [target operating system](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_os).
/// This value is similar to the second and third element of the platform's target triple.
pub fn cargo_cfg_target_os() -> String {
    get_str("CARGO_CFG_TARGET_OS")
}

/// The CPU [target architecture](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_arch).
/// This is similar to the first element of the platform's target triple, but not identical.
pub fn cargo_cfg_target_arch() -> String {
    get_str("CARGO_CFG_TARGET_ARCH")
}

/// The [target vendor](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_vendor).
pub fn cargo_cfg_target_vendor() -> String {
    get_str("CARGO_CFG_TARGET_VENDOR")
}

/// The [target environment](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_env) ABI.
/// This value is similar to the fourth element of the platform's target triple.
///
/// For historical reasons, this value is only defined as not the empty-string when
/// actually needed for disambiguation. Thus, for example, on many GNU platforms,
/// this value will be empty.
pub fn cargo_cfg_target_env() -> String {
    get_str("CARGO_CFG_TARGET_ENV")
}

/// The CPU [pointer width](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_pointer_width).
pub fn cargo_cfg_target_pointer_width() -> u32 {
    get_num("CARGO_CFG_TARGET_POINTER_WIDTH")
}

/// The CPU [target endianness](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_endian).
pub fn cargo_cfg_target_endian() -> String {
    get_str("CARGO_CFG_TARGET_ENDIAN")
}

/// List of CPU [target features](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_feature) enabled.
pub fn cargo_cfg_target_feature() -> Vec<String> {
    get_cfg("target_feature")
}

/// List of CPU [supported atomic widths](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_has_atomic).
pub fn cargo_cfg_target_has_atomic() -> Vec<String> {
    get_cfg("target_has_atomic")
}

/// List of atomic widths that have equal alignment requirements.
///
#[doc = unstable!(cfg_target_has_atomic_equal_alignment, 93822)]
#[cfg(feature = "unstable")]
pub fn cargo_cfg_target_has_atomic_equal_alignment() -> Vec<String> {
    get_cfg("target_has_atomic_equal_alignment")
}

/// List of atomic widths that have atomic load and store operations.
///
#[doc = unstable!(cfg_target_has_atomic_load_store, 94039)]
#[cfg(feature = "unstable")]
pub fn cargo_cfg_target_has_atomic_load_store() -> Vec<String> {
    get_cfg("target_has_atomic_load_store")
}

/// If the target supports thread-local storage.
///
#[doc = unstable!(cfg_target_thread_local, 29594)]
#[cfg(feature = "unstable")]
pub fn cargo_cfg_target_thread_local() -> bool {
    get_bool("CARGO_CFG_TARGET_THREAD_LOCAL")
}

/// The [panic strategy](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#panic).
pub fn cargo_cfg_panic() -> String {
    get_str("CARGO_CFG_PANIC")
}

/// If we are compiling with debug assertions enabled.
pub fn cargo_cfg_debug_assertions() -> bool {
    get_bool("CARGO_CFG_DEBUG_ASSERTIONS")
}

/// If we are compiling with overflow checks enabled.
///
#[doc = unstable!(cfg_overflow_checks, 111466)]
#[cfg(feature = "unstable")]
pub fn cargo_cfg_overflow_checks() -> bool {
    get_bool("CARGO_CFG_OVERFLOW_CHECKS")
}

/// If we are compiling with UB checks enabled.
///
#[doc = unstable!(cfg_ub_checks, 123499)]
#[cfg(feature = "unstable")]
pub fn cargo_cfg_ub_checks() -> bool {
    get_bool("CARGO_CFG_UB_CHECKS")
}

/// The target relocation model.
///
#[doc = unstable!(cfg_relocation_model, 114929)]
#[cfg(feature = "unstable")]
pub fn cargo_cfg_relocation_model() -> String {
    get_str("CARGO_CFG_RELOCATION_MODEL")
}

/// The folder in which all output and intermediate artifacts should be placed.
/// This folder is inside the build directory for the package being built, and
/// it is unique for the package in question.
pub fn out_dir() -> PathBuf {
    get_path("OUT_DIR")
}

/// The [target triple] that is being compiled for. Native code should be compiled
///  for this triple.
///
/// [target triple]: https://doc.rust-lang.org/stable/cargo/appendix/glossary.html#target
pub fn target() -> String {
    get_str("TARGET")
}

/// The host triple of the Rust compiler.
pub fn host() -> String {
    get_str("HOST")
}

/// The parallelism specified as the top-level parallelism. This can be useful to
/// pass a `-j` parameter to a system like `make`. Note that care should be taken
/// when interpreting this value. For historical purposes this is still provided
/// but Cargo, for example, does not need to run `make -j`, and instead can set the
/// `MAKEFLAGS` env var to the content of `CARGO_MAKEFLAGS` to activate the use of
/// Cargo’s GNU Make compatible [jobserver] for sub-make invocations.
///
/// [jobserver]: https://www.gnu.org/software/make/manual/html_node/Job-Slots.html
pub fn num_jobs() -> u32 {
    get_num("NUM_JOBS")
}

/// The [level of optimization](https://doc.rust-lang.org/stable/cargo/reference/profiles.html#opt-level).
pub fn opt_level() -> String {
    get_str("OPT_LEVEL")
}

/// The amount of [debug information](https://doc.rust-lang.org/stable/cargo/reference/profiles.html#debug) included.
pub fn debug() -> String {
    get_str("DEBUG")
}

/// `release` for release builds, `debug` for other builds. This is determined based
/// on if the [profile] inherits from the [`dev`] or [`release`] profile. Using this
/// function is not recommended. Using other functions like [`opt_level`] provides
/// a more correct view of the actual settings being used.
///
/// [profile]: https://doc.rust-lang.org/stable/cargo/reference/profiles.html
/// [`dev`]: https://doc.rust-lang.org/stable/cargo/reference/profiles.html#dev
/// [`release`]: https://doc.rust-lang.org/stable/cargo/reference/profiles.html#release
pub fn profile() -> String {
    get_str("PROFILE")
}

/// [Metadata] set by dependencies. For more information, see build script
/// documentation about [the `links` manifest key][links].
///
/// [metadata]: crate::output::metadata
/// [links]: https://doc.rust-lang.org/stable/cargo/reference/build-scripts.html#the-links-manifest-key
pub fn dep(name: &str, key: &str) -> Option<String> {
    let name = name.to_uppercase().replace('-', "_");
    let key = key.to_uppercase().replace('-', "_");
    let key = format!("DEP_{name}_{key}");
    get_opt_str(&key)
}

/// The compiler that Cargo has resolved to use.
pub fn rustc() -> PathBuf {
    get_path("RUSTC")
}

/// The documentation generator that Cargo has resolved to use.
pub fn rustdoc() -> PathBuf {
    get_path("RUSTDOC")
}

/// The rustc wrapper, if any, that Cargo is using. See [`build.rustc-wrapper`].
///
/// [`build.rustc-wrapper`]: https://doc.rust-lang.org/stable/cargo/reference/config.html#buildrustc-wrapper
pub fn rustc_wrapper() -> Option<PathBuf> {
    get_opt_path("RUSTC_WRAPPER")
}

/// The rustc wrapper, if any, that Cargo is using for workspace members. See
/// [`build.rustc-workspace-wrapper`].
///
/// [`build.rustc-workspace-wrapper`]: https://doc.rust-lang.org/stable/cargo/reference/config.html#buildrustc-workspace-wrapper
pub fn rustc_workspace_wrapper() -> Option<PathBuf> {
    get_opt_path("RUSTC_WORKSPACE_WRAPPER")
}

/// The linker that Cargo has resolved to use for the current target, if specified.
///
/// [`target.*.linker`]: https://doc.rust-lang.org/stable/cargo/reference/config.html#targettriplelinker
pub fn rustc_linker() -> Option<PathBuf> {
    get_opt_path("RUSTC_LINKER")
}

/// Extra flags that Cargo invokes rustc with. See [`build.rustflags`].
///
/// [`build.rustflags`]: https://doc.rust-lang.org/stable/cargo/reference/config.html#buildrustflags
pub fn cargo_encoded_rustflags() -> Vec<String> {
    get_str("CARGO_ENCODED_RUSTFLAGS")
        .split('\x1f')
        .map(str::to_owned)
        .collect()
}

/// The full version of your package.
pub fn cargo_pkg_version() -> String {
    get_str("CARGO_PKG_VERSION")
}

/// The major version of your package.
pub fn cargo_pkg_version_major() -> u64 {
    get_num("CARGO_PKG_VERSION_MAJOR")
}

/// The minor version of your package.
pub fn cargo_pkg_version_minor() -> u64 {
    get_num("CARGO_PKG_VERSION_MINOR")
}

/// The patch version of your package.
pub fn cargo_pkg_version_patch() -> u64 {
    get_num("CARGO_PKG_VERSION_PATCH")
}

/// The pre-release version of your package.
pub fn cargo_pkg_version_pre() -> String {
    get_str("CARGO_PKG_VERSION_PRE")
}

/// Colon separated list of authors from the manifest of your package.
pub fn cargo_pkg_authors() -> Vec<String> {
    get_str("CARGO_PKG_AUTHORS")
        .split(':')
        .map(str::to_owned)
        .collect()
}

/// The name of your package.
pub fn cargo_pkg_name() -> String {
    get_str("CARGO_PKG_NAME")
}

/// The description from the manifest of your package.
pub fn cargo_pkg_description() -> String {
    get_str("CARGO_PKG_DESCRIPTION")
}

/// The Rust version from the manifest of your package. Note that this is the
/// minimum Rust version supported by the package, not the current Rust version.
pub fn cargo_pkg_rust_version() -> String {
    get_str("CARGO_PKG_RUST_VERSION")
}
