//! Inputs from the build system to the build script.
//!
//! This crate does not do any caching or interpreting of the values provided by
//! Cargo beyond the communication protocol itself. It is up to the build script
//! to interpret the string values and decide what to do with them.
//!
//! Reference: <https://doc.rust-lang.org/stable/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts>

use std::path::PathBuf;

use crate::ident::{is_ascii_ident, is_crate_name, is_feature_name};
use crate::output::rerun_if_env_changed;

/// [`ProcessEnv`] wrapper that implicit calls [`rerun_if_env_changed`]
const ENV: RerunIfEnvChanged<ProcessEnv> = RerunIfEnvChanged::new();

/// Abstraction over environment variables
trait Env {
    /// Fetches the environment variable `key`, returning `None` if the variable isn’t set or if
    /// there is another error.
    ///
    /// It may return `None` if the environment variable’s name contains the equal sign character
    /// (`=`) or the NUL character.
    ///
    /// Note that this function will not check if the environment variable is valid Unicode.
    fn get(&self, key: &str) -> Option<std::ffi::OsString>;

    /// Checks the environment variable `key` is present
    ///
    /// It may not be considered present if the environment variable’s name contains the equal sign character
    /// (`=`) or the NUL character.
    fn is_present(&self, key: &str) -> bool;
}

/// Fetches environment variables from the current process
struct ProcessEnv;

impl Env for ProcessEnv {
    fn get(&self, key: &str) -> Option<std::ffi::OsString> {
        std::env::var_os(key)
    }

    fn is_present(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

/// [`Env`] wrapper that implicitly calls [`rerun_if_env_changed`]
struct RerunIfEnvChanged<E: Env>(E);

impl RerunIfEnvChanged<ProcessEnv> {
    const fn new() -> Self {
        Self(ProcessEnv)
    }
}

impl<E: Env> Env for RerunIfEnvChanged<E> {
    #[track_caller]
    fn get(&self, key: &str) -> Option<std::ffi::OsString> {
        rerun_if_env_changed(key);
        self.0.get(key)
    }

    #[track_caller]
    fn is_present(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

/// Path to the `cargo` binary performing the build.
#[track_caller]
pub fn cargo() -> PathBuf {
    to_path(var_or_panic("CARGO"))
}

/// The directory containing the manifest for the package being built (the package
/// containing the build script).
///
/// Also note that this is the value of the current
/// working directory of the build script when it starts.
#[track_caller]
pub fn cargo_manifest_dir() -> PathBuf {
    to_path(var_or_panic("CARGO_MANIFEST_DIR"))
}

/// The path to the manifest of your package.
#[track_caller]
pub fn cargo_manifest_path() -> PathBuf {
    ENV.get("CARGO_MANIFEST_PATH")
        .map(to_path)
        .unwrap_or_else(|| {
            let mut path = cargo_manifest_dir();
            path.push("Cargo.toml");
            path
        })
}

/// The manifest `links` value.
#[track_caller]
pub fn cargo_manifest_links() -> Option<String> {
    ENV.get("CARGO_MANIFEST_LINKS").map(to_string)
}

/// Contains parameters needed for Cargo’s [jobserver] implementation to parallelize
/// subprocesses.
///
/// Rustc or cargo invocations from build.rs can already read
/// `CARGO_MAKEFLAGS`, but GNU Make requires the flags to be specified either
/// directly as arguments, or through the `MAKEFLAGS` environment variable.
/// Currently Cargo doesn’t set the `MAKEFLAGS` variable, but it’s free for build
/// scripts invoking GNU Make to set it to the contents of `CARGO_MAKEFLAGS`.
///
/// [jobserver]: https://www.gnu.org/software/make/manual/html_node/Job-Slots.html
#[track_caller]
pub fn cargo_makeflags() -> Option<String> {
    ENV.get("CARGO_MAKEFLAGS").map(to_string)
}

/// For each activated feature of the package being built, this will be `true`.
#[track_caller]
pub fn cargo_feature(name: &str) -> bool {
    if !is_feature_name(name) {
        panic!("invalid feature name {name:?}")
    }
    let name = name.to_uppercase().replace('-', "_");
    let key = format!("CARGO_FEATURE_{name}");
    ENV.is_present(&key)
}

/// For each [configuration option] of the package being built, this will contain
/// the value of the configuration.
///
/// This includes values built-in to the compiler
/// (which can be seen with `rustc --print=cfg`) and values set by build scripts
/// and extra flags passed to rustc (such as those defined in `RUSTFLAGS`).
///
/// [configuration option]: https://doc.rust-lang.org/stable/reference/conditional-compilation.html
#[track_caller]
pub fn cargo_cfg(cfg: &str) -> Option<Vec<String>> {
    let var = cargo_cfg_var(cfg);
    ENV.get(&var).map(|v| to_strings(v, ','))
}

#[track_caller]
fn cargo_cfg_var(cfg: &str) -> String {
    if !is_ascii_ident(cfg) {
        panic!("invalid configuration option {cfg:?}")
    }
    let cfg = cfg.to_uppercase().replace('-', "_");
    let key = format!("CARGO_CFG_{cfg}");
    key
}

pub use self::cfg::*;
mod cfg {
    use super::*;

    // those disabled with #[cfg(any())] don't seem meaningfully useful
    // but we list all cfg that are default known to check-cfg

    /// Each activated feature of the package being built
    #[doc = requires_msrv!("1.85")]
    #[track_caller]
    pub fn cargo_cfg_feature() -> Vec<String> {
        to_strings(var_or_panic(&cargo_cfg_var("feature")), ',')
    }

    #[cfg(any())]
    #[track_caller]
    pub fn cargo_cfg_clippy() -> bool {
        ENV.is_present("CARGO_CFG_CLIPPY")
    }

    /// If we are compiling with debug assertions enabled.
    ///
    /// Build scripts are not passed this cfg because
    /// this cfg is always true and misleading.
    /// That is because Cargo queries rustc without any profile settings.
    #[cfg(any())]
    #[track_caller]
    pub fn cargo_cfg_debug_assertions() -> bool {
        ENV.is_present("CARGO_CFG_DEBUG_ASSERTIONS")
    }

    #[cfg(any())]
    #[track_caller]
    pub fn cargo_cfg_doc() -> bool {
        ENV.is_present("CARGO_CFG_DOC")
    }

    #[cfg(any())]
    #[track_caller]
    pub fn cargo_cfg_docsrs() -> bool {
        ENV.is_present("CARGO_CFG_DOCSRS")
    }

    #[cfg(any())]
    #[track_caller]
    pub fn cargo_cfg_doctest() -> bool {
        ENV.is_present("CARGO_CFG_DOCTEST")
    }

    /// The level of detail provided by derived [`Debug`] implementations.
    #[doc = unstable!(fmt_dbg, 129709)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_fmt_debug() -> String {
        to_string(var_or_panic("CARGO_CFG_FMT_DEBUG"))
    }

    #[cfg(any())]
    #[track_caller]
    pub fn cargo_cfg_miri() -> bool {
        ENV.is_present("CARGO_CFG_MIRI")
    }

    /// If we are compiling with overflow checks enabled.
    #[doc = unstable!(cfg_overflow_checks, 111466)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_overflow_checks() -> bool {
        ENV.is_present("CARGO_CFG_OVERFLOW_CHECKS")
    }

    /// The [panic strategy](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#panic).
    #[track_caller]
    pub fn cargo_cfg_panic() -> String {
        to_string(var_or_panic("CARGO_CFG_PANIC"))
    }

    /// If the crate is being compiled as a procedural macro.
    #[track_caller]
    pub fn cargo_cfg_proc_macro() -> bool {
        ENV.is_present("CARGO_CFG_PROC_MACRO")
    }

    /// The target relocation model.
    #[doc = unstable!(cfg_relocation_model, 114929)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_relocation_model() -> String {
        to_string(var_or_panic("CARGO_CFG_RELOCATION_MODEL"))
    }

    #[cfg(any())]
    #[track_caller]
    pub fn cargo_cfg_rustfmt() -> bool {
        ENV.is_present("CARGO_CFG_RUSTFMT")
    }

    /// Sanitizers enabled for the crate being compiled.
    #[doc = unstable!(cfg_sanitize, 39699)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_sanitize() -> Option<Vec<String>> {
        ENV.get("CARGO_CFG_SANITIZE").map(|v| to_strings(v, ','))
    }

    /// If CFI sanitization is generalizing pointers.
    #[doc = unstable!(cfg_sanitizer_cfi, 89653)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_sanitizer_cfi_generalize_pointers() -> bool {
        ENV.is_present("CARGO_CFG_SANITIZER_CFI_GENERALIZE_POINTERS")
    }

    /// If CFI sanitization is normalizing integers.
    #[doc = unstable!(cfg_sanitizer_cfi, 89653)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_sanitizer_cfi_normalize_integers() -> bool {
        ENV.is_present("CARGO_CFG_SANITIZER_CFI_NORMALIZE_INTEGERS")
    }

    /// Disambiguation of the [target ABI](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_abi)
    /// when the [target env](cargo_cfg_target_env) isn't sufficient.
    ///
    /// For historical reasons, this value is only defined as `Some` when
    /// actually needed for disambiguation. Thus, for example, on many GNU platforms,
    /// this value will be `None`.
    #[track_caller]
    pub fn cargo_cfg_target_abi() -> Option<String> {
        to_opt(var_or_panic("CARGO_CFG_TARGET_ABI")).map(to_string)
    }

    /// The CPU [target architecture](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_arch).
    /// This is similar to the first element of the platform's target triple, but not identical.
    #[track_caller]
    pub fn cargo_cfg_target_arch() -> String {
        to_string(var_or_panic("CARGO_CFG_TARGET_ARCH"))
    }

    /// The CPU [target endianness](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_endian).
    #[track_caller]
    pub fn cargo_cfg_target_endian() -> String {
        to_string(var_or_panic("CARGO_CFG_TARGET_ENDIAN"))
    }

    /// The [target environment](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_env) ABI.
    /// This value is similar to the fourth element of the platform's target triple.
    ///
    /// For historical reasons, this value is only defined as not the empty-string when
    /// actually needed for disambiguation. Thus, for example, on many GNU platforms,
    /// this value will be empty.
    #[track_caller]
    pub fn cargo_cfg_target_env() -> String {
        to_string(var_or_panic("CARGO_CFG_TARGET_ENV"))
    }

    /// The [target family](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_family).
    #[track_caller]
    pub fn cargo_target_family() -> Vec<String> {
        to_strings(var_or_panic(&cargo_cfg_var("target_family")), ',')
    }

    /// List of CPU [target features](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_feature) enabled.
    #[track_caller]
    pub fn cargo_cfg_target_feature() -> Vec<String> {
        to_strings(var_or_panic(&cargo_cfg_var("target_feature")), ',')
    }

    /// List of CPU [supported atomic widths](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_has_atomic).
    #[track_caller]
    pub fn cargo_cfg_target_has_atomic() -> Vec<String> {
        to_strings(var_or_panic(&cargo_cfg_var("target_has_atomic")), ',')
    }

    /// List of atomic widths that have equal alignment requirements.
    #[doc = unstable!(cfg_target_has_atomic_equal_alignment, 93822)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_target_has_atomic_equal_alignment() -> Vec<String> {
        to_strings(
            var_or_panic(&cargo_cfg_var("target_has_atomic_equal_alignment")),
            ',',
        )
    }

    /// List of atomic widths that have atomic load and store operations.
    #[doc = unstable!(cfg_target_has_atomic_load_store, 94039)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_target_has_atomic_load_store() -> Vec<String> {
        to_strings(
            var_or_panic(&cargo_cfg_var("target_has_atomic_load_store")),
            ',',
        )
    }

    /// The [target operating system](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_os).
    /// This value is similar to the second and third element of the platform's target triple.
    #[track_caller]
    pub fn cargo_cfg_target_os() -> String {
        to_string(var_or_panic("CARGO_CFG_TARGET_OS"))
    }

    /// The CPU [pointer width](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_pointer_width).
    #[track_caller]
    pub fn cargo_cfg_target_pointer_width() -> u32 {
        to_parsed(var_or_panic("CARGO_CFG_TARGET_POINTER_WIDTH"))
    }

    /// If the target supports thread-local storage.
    #[doc = unstable!(cfg_target_thread_local, 29594)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_target_thread_local() -> bool {
        ENV.is_present("CARGO_CFG_TARGET_THREAD_LOCAL")
    }

    /// The [target vendor](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#target_vendor).
    #[track_caller]
    pub fn cargo_cfg_target_vendor() -> String {
        to_string(var_or_panic("CARGO_CFG_TARGET_VENDOR"))
    }

    #[cfg(any())]
    #[track_caller]
    pub fn cargo_cfg_test() -> bool {
        ENV.is_present("CARGO_CFG_TEST")
    }

    /// If we are compiling with UB checks enabled.
    #[doc = unstable!(cfg_ub_checks, 123499)]
    #[cfg(feature = "unstable")]
    #[track_caller]
    pub fn cargo_cfg_ub_checks() -> bool {
        ENV.is_present("CARGO_CFG_UB_CHECKS")
    }

    /// Set on [unix-like platforms](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#unix-and-windows).
    #[track_caller]
    pub fn cargo_cfg_unix() -> bool {
        ENV.is_present("CARGO_CFG_UNIX")
    }

    /// Set on [windows-like platforms](https://doc.rust-lang.org/stable/reference/conditional-compilation.html#unix-and-windows).
    #[track_caller]
    pub fn cargo_cfg_windows() -> bool {
        ENV.is_present("CARGO_CFG_WINDOWS")
    }
}

/// The folder in which all output and intermediate artifacts should be placed.
///
/// This folder is inside the build directory for the package being built, and
/// it is unique for the package in question.
#[track_caller]
pub fn out_dir() -> PathBuf {
    to_path(var_or_panic("OUT_DIR"))
}

/// The [target triple] that is being compiled for. Native code should be compiled
///  for this triple.
///
/// [target triple]: https://doc.rust-lang.org/stable/cargo/appendix/glossary.html#target
#[track_caller]
pub fn target() -> String {
    to_string(var_or_panic("TARGET"))
}

/// The host triple of the Rust compiler.
#[track_caller]
pub fn host() -> String {
    to_string(var_or_panic("HOST"))
}

/// The parallelism specified as the top-level parallelism.
///
/// This can be useful to
/// pass a `-j` parameter to a system like `make`. Note that care should be taken
/// when interpreting this value. For historical purposes this is still provided
/// but Cargo, for example, does not need to run `make -j`, and instead can set the
/// `MAKEFLAGS` env var to the content of `CARGO_MAKEFLAGS` to activate the use of
/// Cargo’s GNU Make compatible [jobserver] for sub-make invocations.
///
/// [jobserver]: https://www.gnu.org/software/make/manual/html_node/Job-Slots.html
#[track_caller]
pub fn num_jobs() -> u32 {
    to_parsed(var_or_panic("NUM_JOBS"))
}

/// The [level of optimization](https://doc.rust-lang.org/stable/cargo/reference/profiles.html#opt-level).
#[track_caller]
pub fn opt_level() -> String {
    to_string(var_or_panic("OPT_LEVEL"))
}

/// The amount of [debug information](https://doc.rust-lang.org/stable/cargo/reference/profiles.html#debug) included.
#[track_caller]
pub fn debug() -> String {
    to_string(var_or_panic("DEBUG"))
}

/// `release` for release builds, `debug` for other builds.
///
/// This is determined based
/// on if the [profile] inherits from the [`dev`] or [`release`] profile. Using this
/// function is not recommended. Using other functions like [`opt_level`] provides
/// a more correct view of the actual settings being used.
///
/// [profile]: https://doc.rust-lang.org/stable/cargo/reference/profiles.html
/// [`dev`]: https://doc.rust-lang.org/stable/cargo/reference/profiles.html#dev
/// [`release`]: https://doc.rust-lang.org/stable/cargo/reference/profiles.html#release
#[track_caller]
pub fn profile() -> String {
    to_string(var_or_panic("PROFILE"))
}

/// [Metadata] set by dependencies. For more information, see build script
/// documentation about [the `links` manifest key][links].
///
/// [metadata]: crate::output::metadata
/// [links]: https://doc.rust-lang.org/stable/cargo/reference/build-scripts.html#the-links-manifest-key
#[track_caller]
pub fn dep_metadata(name: &str, key: &str) -> Option<String> {
    if !is_crate_name(name) {
        panic!("invalid dependency name {name:?}")
    }
    if !is_ascii_ident(key) {
        panic!("invalid metadata key {key:?}")
    }

    let name = name.to_uppercase().replace('-', "_");
    let key = key.to_uppercase().replace('-', "_");
    let key = format!("DEP_{name}_{key}");
    ENV.get(&key).map(to_string)
}

/// The compiler that Cargo has resolved to use.
#[track_caller]
pub fn rustc() -> PathBuf {
    to_path(var_or_panic("RUSTC"))
}

/// The documentation generator that Cargo has resolved to use.
#[track_caller]
pub fn rustdoc() -> PathBuf {
    to_path(var_or_panic("RUSTDOC"))
}

/// The rustc wrapper, if any, that Cargo is using. See [`build.rustc-wrapper`].
///
/// [`build.rustc-wrapper`]: https://doc.rust-lang.org/stable/cargo/reference/config.html#buildrustc-wrapper
#[track_caller]
pub fn rustc_wrapper() -> Option<PathBuf> {
    ENV.get("RUSTC_WRAPPER").map(to_path)
}

/// The rustc wrapper, if any, that Cargo is using for workspace members. See
/// [`build.rustc-workspace-wrapper`].
///
/// [`build.rustc-workspace-wrapper`]: https://doc.rust-lang.org/stable/cargo/reference/config.html#buildrustc-workspace-wrapper
#[track_caller]
pub fn rustc_workspace_wrapper() -> Option<PathBuf> {
    ENV.get("RUSTC_WORKSPACE_WRAPPER").map(to_path)
}

/// The linker that Cargo has resolved to use for the current target, if specified.
///
/// [`target.*.linker`]: https://doc.rust-lang.org/stable/cargo/reference/config.html#targettriplelinker
#[track_caller]
pub fn rustc_linker() -> Option<PathBuf> {
    ENV.get("RUSTC_LINKER").map(to_path)
}

/// Extra flags that Cargo invokes rustc with. See [`build.rustflags`].
///
/// [`build.rustflags`]: https://doc.rust-lang.org/stable/cargo/reference/config.html#buildrustflags
#[track_caller]
pub fn cargo_encoded_rustflags() -> Vec<String> {
    to_strings(var_or_panic("CARGO_ENCODED_RUSTFLAGS"), '\x1f')
}

/// The full version of your package.
#[track_caller]
pub fn cargo_pkg_version() -> String {
    to_string(var_or_panic("CARGO_PKG_VERSION"))
}

/// The major version of your package.
#[track_caller]
pub fn cargo_pkg_version_major() -> u64 {
    to_parsed(var_or_panic("CARGO_PKG_VERSION_MAJOR"))
}

/// The minor version of your package.
#[track_caller]
pub fn cargo_pkg_version_minor() -> u64 {
    to_parsed(var_or_panic("CARGO_PKG_VERSION_MINOR"))
}

/// The patch version of your package.
#[track_caller]
pub fn cargo_pkg_version_patch() -> u64 {
    to_parsed(var_or_panic("CARGO_PKG_VERSION_PATCH"))
}

/// The pre-release version of your package.
#[track_caller]
pub fn cargo_pkg_version_pre() -> Option<String> {
    to_opt(var_or_panic("CARGO_PKG_VERSION_PRE")).map(to_string)
}

/// The authors from the manifest of your package.
#[track_caller]
pub fn cargo_pkg_authors() -> Vec<String> {
    to_strings(var_or_panic("CARGO_PKG_AUTHORS"), ':')
}

/// The name of your package.
#[track_caller]
pub fn cargo_pkg_name() -> String {
    to_string(var_or_panic("CARGO_PKG_NAME"))
}

/// The description from the manifest of your package.
#[track_caller]
pub fn cargo_pkg_description() -> Option<String> {
    to_opt(var_or_panic("CARGO_PKG_DESCRIPTION")).map(to_string)
}

/// The home page from the manifest of your package.
#[track_caller]
pub fn cargo_pkg_homepage() -> Option<String> {
    to_opt(var_or_panic("CARGO_PKG_HOMEPAGE")).map(to_string)
}

/// The repository from the manifest of your package.
#[track_caller]
pub fn cargo_pkg_repository() -> Option<String> {
    to_opt(var_or_panic("CARGO_PKG_REPOSITORY")).map(to_string)
}

/// The license from the manifest of your package.
#[track_caller]
pub fn cargo_pkg_license() -> Option<String> {
    to_opt(var_or_panic("CARGO_PKG_LICENSE")).map(to_string)
}

/// The license file from the manifest of your package.
#[track_caller]
pub fn cargo_pkg_license_file() -> Option<PathBuf> {
    to_opt(var_or_panic("CARGO_PKG_LICENSE_FILE")).map(to_path)
}

/// The Rust version from the manifest of your package. Note that this is the
/// minimum Rust version supported by the package, not the current Rust version.
#[track_caller]
pub fn cargo_pkg_rust_version() -> Option<String> {
    to_opt(var_or_panic("CARGO_PKG_RUST_VERSION")).map(to_string)
}

/// Path to the README file of your package.
#[track_caller]
pub fn cargo_pkg_readme() -> Option<PathBuf> {
    to_opt(var_or_panic("CARGO_PKG_README")).map(to_path)
}

#[track_caller]
fn var_or_panic(key: &str) -> std::ffi::OsString {
    ENV.get(key)
        .unwrap_or_else(|| panic!("cargo environment variable `{key}` is missing"))
}

fn to_path(value: std::ffi::OsString) -> PathBuf {
    PathBuf::from(value)
}

#[track_caller]
fn to_string(value: std::ffi::OsString) -> String {
    match value.into_string() {
        Ok(s) => s,
        Err(value) => {
            let err = std::str::from_utf8(value.as_encoded_bytes()).unwrap_err();
            panic!("{err}")
        }
    }
}

fn to_opt(value: std::ffi::OsString) -> Option<std::ffi::OsString> {
    (!value.is_empty()).then_some(value)
}

#[track_caller]
fn to_strings(value: std::ffi::OsString, sep: char) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    let value = to_string(value);
    value.split(sep).map(str::to_owned).collect()
}

#[track_caller]
fn to_parsed<T>(value: std::ffi::OsString) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = to_string(value);
    match value.parse() {
        Ok(s) => s,
        Err(err) => {
            panic!("{err}")
        }
    }
}
