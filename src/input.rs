use std::{env, ffi::OsString, path::PathBuf};

macro_rules! export {
    () => {};
    ($(#[$meta:meta])* $f:ident -> String = $var:ident $(; $($rest:tt)*)? ) => {
        $(#[$meta])*
        pub fn $f() -> String {
            env::var_os(stringify!($var))
                .expect(concat!("cargo buildscript env var $", stringify!($var), " not found"))
                .into_string()
                .expect(concat!("cargo buildscript env var $", stringify!($var), " contained invalid UTF-8"))
        }

        $(export! {
            $($rest)*
        })?
    };
    ($(#[$meta:meta])* $f:ident -> Option<String> = $var:ident $(; $($rest:tt)*)? ) => {
        $(#[$meta])*
        pub fn $f() -> Option<String> {
            env::var_os(stringify!($var)).map(|it| it
                .into_string()
                .expect(concat!("cargo buildscript env var $", stringify!($var), " contained invalid UTF-8"))
            )
        }

        $(export! {
            $($rest)*
        })?
    };
    ($(#[$meta:meta])* $f:ident -> usize = $var:ident $(; $($rest:tt)*)? ) => {
        $(#[$meta])*
        pub fn $f() -> usize {
            env::var_os(stringify!($var))
                .expect(concat!("cargo buildscript env var $", stringify!($var), " not found"))
                .into_string()
                .expect(concat!("cargo buildscript env var $", stringify!($var), " contained invalid UTF-8"))
                .parse()
                .expect(concat!("cargo buildscript env var $", stringify!($var), " did not parse as `usize`"))
        }

        $(export! {
            $($rest)*
        })?
    };
    ($(#[$meta:meta])* $f:ident -> Vec<String> = $var:ident $(; $($rest:tt)*)? ) => {
        $(#[$meta])*
        pub fn $f() -> Vec<String> {
            env::var_os(stringify!($var))
                .expect(concat!("cargo buildscript env var $", stringify!($var), " not found"))
                .into_string()
                .expect(concat!("cargo buildscript env var $", stringify!($var), " contained invalid UTF-8"))
                .split(',')
                .map(Into::into)
                .collect()
        }

        $(export! {
            $($rest)*
        })?
    };
    ($(#[$meta:meta])* $f:ident -> bool = $var:ident $(; $($rest:tt)*)? ) => {
        $(#[$meta])*
        pub fn $f() -> bool {
            env::var_os(stringify!($var))
                .is_some()
        }

        $(export! {
            $($rest)*
        })?
    };
    ($(#[$meta:meta])* $f:ident -> PathBuf = $var:ident $(; $($rest:tt)*)? ) => {
        $(#[$meta])*
        pub fn $f() -> PathBuf {
            env::var_os(stringify!($var))
                .expect(concat!("cargo buildscript env var $", stringify!($var), " not found"))
                .into()
        }

        $(export! {
            $($rest)*
        })?
    };
    ($(#[$meta:meta])* $f:ident -> Option<PathBuf> = $var:ident $(; $($rest:tt)*)? ) => {
        $(#[$meta])*
        pub fn $f() -> Option<PathBuf> {
            env::var_os(stringify!($var))
                .map(Into::into)
        }

        $(export! {
            $($rest)*
        })?
    };
    ($(#[$meta:meta])* $f:ident -> OsString = $var:ident $(; $($rest:tt)*)? ) => {
        $(#[$meta])*
        pub fn $f() -> OsString {
            env::var_os(stringify!($var))
                .expect(concat!("cargo buildscript env var $", stringify!($var), " not found"))
        }

        $(export! {
            $($rest)*
        })?
    };
    ($(#[$meta:meta])* $f:ident -> $out:ty = $var:ident $(; $($rest:tt)*)? ) => {
        compile_error!(concat!("Provided unknown output type ", stringify!($out), " to export!"));

        $(export! {
            $($rest)*
        })?
    };
}

export! {
    /// Path to the `cargo` binary performing the build.
    cargo -> PathBuf = CARGO;
    /// The directory containing the manifest for the package being built (the
    /// package containing the build script). Also note that this is the value
    /// of the current working directory of the build script when it starts.
    cargo_manifest_dir -> PathBuf = CARGO_MANIFEST_DIR;
    /// The manifest links value.
    cargo_manifest_links -> String = CARGO_MANIFEST_LINKS;
    /// Contains parameters needed for Cargo's jobserver implementation to
    /// parallelize subprocesses. Rustc or cargo invocations from build.rs
    /// can already read CARGO_MAKEFLAGS, but GNU Make requires the flags
    /// to be specified either directly as arguments, or through the MAKEFLAGS
    /// environment variable. Currently Cargo doesn't set the MAKEFLAGS
    /// variable, but it's free for build scripts invoking GNU Make to set it
    /// to the contents of CARGO_MAKEFLAGS.
    cargo_makeflags -> OsString = CARGO_MAKEFLAGS;
    /// Set on [unix-like platforms](https://doc.rust-lang.org/reference/conditional-compilation.html#unix-and-windows).
    cargo_cfg_unix -> bool = CARGO_CFG_UNIX;
    /// Set on [windows-like platforms](https://doc.rust-lang.org/reference/conditional-compilation.html#unix-and-windows).
    cargo_cfg_windows -> bool = CARGO_CFG_WINDOWS;
    /// The [target family](https://doc.rust-lang.org/reference/conditional-compilation.html#target_family).
    cargo_cfg_target_family -> Vec<String> = CARGO_CFG_TARGET_FAMILY;
    /// The [target operating system](https://doc.rust-lang.org/reference/conditional-compilation.html#target_os).
    cargo_cfg_target_os -> String = CARGO_CFG_TARGET_OS;
    /// The CPU [target architecture](https://doc.rust-lang.org/reference/conditional-compilation.html#target_arch).
    cargo_cfg_target_arch -> String = CARGO_CFG_TARGET_ARCH;
    /// The [target vendor](https://doc.rust-lang.org/reference/conditional-compilation.html#target_vendor).
    cargo_cfg_target_vendor -> String = CARGO_CFG_TARGET_VENDOR;
    /// The [target environment](https://doc.rust-lang.org/reference/conditional-compilation.html#target_env) ABI.
    cargo_cfg_target_env -> String = CARGO_CFG_TARGET_ENV;
    /// The CPU [pointer width](https://doc.rust-lang.org/reference/conditional-compilation.html#target_pointer_width).
    cargo_cfg_pointer_width -> usize = CARGO_CFG_TARGET_POINTER_WIDTH;
    /// Teh CPU [target endianness](https://doc.rust-lang.org/reference/conditional-compilation.html#target_endian).
    cargo_cfg_target_endian -> String = CARGO_CFG_TARGET_ENDIAN;
    /// List of CPU [target features](https://doc.rust-lang.org/reference/conditional-compilation.html#target_feature) enabled.
    cargo_cfg_target_feature -> Vec<String> = CARGO_CFG_TARGET_FEATURE;
    /// The folder in which all output should be placed. This folder is inside
    /// the build directory for the package being built, and it is unique for
    /// the package in question.
    out_dir -> PathBuf = OUT_DIR;
    /// The target triple that is being compiled for. Native code should be
    /// compiled for this triple. See the [Target Triple] description for
    /// more information.
    ///
    /// [Target Triple]: https://doc.rust-lang.org/cargo/appendix/glossary.html#target
    target -> String = TARGET;
    /// The host triple of the Rust compiler.
    host -> String = HOST;
    /// The parallelism specified as the top-level parallelism. This can be
    /// useful to pass a `-j` parameter to a system like `make`. Note that care
    /// should be taken when interpreting this environment variable. For
    /// historical purposes this is still provided but recent versions of
    /// Cargo, for example, do not need to run `make -j`, and instead can set
    /// the `MAKEFLAGS` env var to the content of `CARGO_MAKEFLAGS` to activate
    /// the use of Cargo's GNU Make compatible [jobserver] for sub-make
    /// invocations.
    ///
    /// [jobserver]: https://www.gnu.org/software/make/manual/html_node/Job-Slots.html
    num_jobs -> String = NUM_JOBS;
    /// Value of the corresponding variable for the profile currently being built.
    opt_level -> String = OPT_LEVEL;
    /// Value of the corresponding variable for the profile currently being built.
    debug -> String = DEBUG;
    /// `release` for release builds, `debug` for other builds. This is
    /// determined based on if the profile inherits from the [`dev`] or
    /// [`release`] profile. Using this environment variable is not
    /// recommended. Using other environment variables like `OPT_LEVEL`
    /// provide a more correct view of the actual settings being used.
    ///
    /// [`dev`]: https://doc.rust-lang.org/cargo/reference/profiles.html#dev
    /// [`release`]: https://doc.rust-lang.org/cargo/reference/profiles.html#release
    profile -> String = PROFILE;
    /// The compiler that Cargo has resolved to use, passed to the build script
    /// so it might use it as well.
    rustc -> PathBuf = RUSTC;
    /// The documentation generator that Cargo has resolved to use, passed to
    /// the build script so it might use it as well.
    rustdoc -> PathBuf = RUSTDOC;
    /// The `rustc` wrapper, if any, that Cargo is using. See
    /// [`build.rustc-wrapper`](https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-wrapper).
    rustc_wrapper -> Option<PathBuf> = RUSTC_WRAPPER;
    /// The `rustc` wrapper, if any, that Cargo is using for workspace members.
    /// See [`build.rustc-workspace-wrapper`](https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-workspace-wrapper).
    rustc_workspace_wrapper -> Option<PathBuf> = RUSTC_WORKSPACE_WRAPPER;
    /// The path to the linker binary that Cargo has resolved to use for the
    /// current target, if specified.
    rustc_linker -> Option<PathBuf> = RUSTC_LINKER;
    /// The full version of your package.
    cargo_pkg_version -> String = CARGO_PKG_VERSION;
    /// The major version of your package.
    cargo_pkg_version_major -> usize = CARGO_PKG_VERSION_MAJOR;
    /// The minor version of your package.
    cargo_pkg_version_minor -> usize = CARGO_PKG_VERSION_MINOR;
    /// The patch version of your package.
    cargo_pkg_version_patch -> usize = CARGO_PKG_VERSION_PATCH;
    /// The pre-release of your package.
    cargo_pkg_version_pre -> String = CARGO_PKG_VERSION_PRE;
    /// The name of your package.
    cargo_pkg_name -> String = CARGO_PKG_NAME;
    /// The description from the manifest of your package.
    cargo_pkg_description -> String = CARGO_PKG_DESCRIPTION;
    /// The home page from the manifest of your package.
    cargo_pkg_homepage -> String = CARGO_PKG_HOMEPAGE;
    /// The repository from the manifest of your package.
    cargo_pkg_repository -> String = CARGO_PKG_REPOSITORY;
    /// The license from the manifest of your package.
    cargo_pkg_license -> String = CARGO_PKG_LICENSE;
    /// The license file from the manifest of your package.
    cargo_pkg_license_file -> String = CARGO_PKG_LICENSE_FILE;
}

/// For each activated feature of the package being built, this will be true.
pub fn cargo_feature(name: &str) -> bool {
    let key = format!("CARGO_FEATURE_{}", name.to_uppercase().replace('-', "_"));
    env::var_os(key).is_some()
}

/// For each [configuration option] of the package being built, this will
/// contain the value of the configuration. Boolean configurations are present
/// if they are set, and not present otherwise. This includes values built-in
/// to the compiler (which can be seen with `rustc --print=cfg`) and values set
/// by build scripts and extra flags passed to `rustc` (such as those defined
/// in `RUSTFLAGS`).
///
/// [configuration option]: https://doc.rust-lang.org/reference/conditional-compilation.html
pub fn cargo_cfg(cfg: &str) -> Option<Vec<String>> {
    let key = format!("CARGO_CFG_{}", cfg.to_uppercase().replace('-', "_"));
    let val = env::var_os(&key)?.into_string().unwrap_or_else(|_| {
        panic!("cargo buildscript env var ${key} contained invalid UTF-8");
    });
    Some(val.split(',').map(Into::into).collect())
}

/// Each build script can generate an arbitrary set of metadata in the form of
/// key-value pairs. This metadata is passed to the build scripts of
/// **dependent** packages. For example, if the package `bar` depends on `foo`,
/// then if `foo` generates `key=value` as part of its build script metadata,
/// then the build script of `bar` will have the environment variables
/// `DEP_FOO_KEY=value`.
pub fn dep(name: &str, key: &str) -> Option<String> {
    let key = format!(
        "DEP_{}_{}",
        name.to_uppercase().replace('-', "_"),
        key.to_uppercase().replace('-', "_")
    );
    let val = env::var_os(&key)?.into_string().unwrap_or_else(|_| {
        panic!("cargo buildscript env var ${key} contained invalid UTF-8");
    });
    Some(val)
}

/// Extra flags that Cargo invokes rustc with. See [`build.rustflags`]. Note
/// that since Rust 1.55, `RUSTFLAGS` is removed from the environment; scripts
/// should use `CARGO_ENCODED_RUSTFLAGS` instead.
///
/// [`build.rustflags`]: https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags
pub fn cargo_encoded_rustflags() -> Vec<String> {
    let val = env::var_os("CARGO_ENCODED_RUSTFLAGS")
        .expect("cargo buildscript env var $CARGO_ENCODED_RUSTFLAGS")
        .into_string()
        .expect("cargo buildscript env var $CARGO_ENCODED_RUSTFLAGS contained invalid UTF-8");
    val.split('\x1f').map(Into::into).collect()
}

/// List of authors from the manifest of your package.
pub fn cargo_pkg_authors() -> Vec<String> {
    let val = env::var_os("CARGO_PKG_AUTHORS")
        .expect("cargo buildscript env var $CARGO_PKG_AUTHORS")
        .into_string()
        .expect("cargo buildscript env var $CARGO_PKG_AUTHORS contained invalid UTF-8");
    val.split(':').map(Into::into).collect()
}
