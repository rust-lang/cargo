//! Cargo's config system.
//!
//! The [`GlobalContext`] object contains general information about the environment,
//! and provides access to Cargo's configuration files.
//!
//! ## Config value API
//!
//! The primary API for fetching user-defined config values is the
//! [`GlobalContext::get`] method. It uses `serde` to translate config values to a
//! target type.
//!
//! There are a variety of helper types for deserializing some common formats:
//!
//! - [`value::Value`]: This type provides access to the location where the
//!   config value was defined.
//! - [`ConfigRelativePath`]: For a path that is relative to where it is
//!   defined.
//! - [`PathAndArgs`]: Similar to [`ConfigRelativePath`],
//!   but also supports a list of arguments, useful for programs to execute.
//! - [`StringList`]: Get a value that is either a list or a whitespace split
//!   string.
//!
//! # Config schemas
//!
//! Configuration schemas are defined in the [`schema`] module.
//!
//! ## Config deserialization
//!
//! Cargo uses a two-layer deserialization approach:
//!
//! 1. **External sources → `ConfigValue`** ---
//!    Configuration files, environment variables, and CLI `--config` arguments
//!    are parsed into [`ConfigValue`] instances via [`ConfigValue::from_toml`].
//!    These parsed results are stored in [`GlobalContext`].
//!
//! 2. **`ConfigValue` → Target types** ---
//!    The [`GlobalContext::get`] method uses a [custom serde deserializer](Deserializer)
//!    to convert [`ConfigValue`] instances to the caller's desired type.
//!    Precedence between [`ConfigValue`] sources is resolved during retrieval
//!    based on [`Definition`] priority.
//!    See the top-level documentation of the [`de`] module for more.
//!
//! ## Map key recommendations
//!
//! Handling tables that have arbitrary keys can be tricky, particularly if it
//! should support environment variables. In general, if possible, the caller
//! should pass the full key path into the `get()` method so that the config
//! deserializer can properly handle environment variables (which need to be
//! uppercased, and dashes converted to underscores).
//!
//! A good example is the `[target]` table. The code will request
//! `target.$TRIPLE` and the config system can then appropriately fetch
//! environment variables like `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER`.
//! Conversely, it is not possible do the same thing for the `cfg()` target
//! tables (because Cargo must fetch all of them), so those do not support
//! environment variables.
//!
//! Try to avoid keys that are a prefix of another with a dash/underscore. For
//! example `build.target` and `build.target-dir`. This is OK if these are not
//! structs/maps, but if it is a struct or map, then it will not be able to
//! read the environment variable due to ambiguity. (See `ConfigMapAccess` for
//! more details.)

use crate::util::cache_lock::{CacheLock, CacheLockMode, CacheLocker};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File};
use std::io::SeekFrom;
use std::io::prelude::*;
use std::mem;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard, Once, OnceLock};
use std::time::Instant;

use self::ConfigValue as CV;
use crate::core::compiler::rustdoc::RustdocExternMap;
use crate::core::global_cache_tracker::{DeferredGlobalLastUse, GlobalCacheTracker};
use crate::core::shell::Verbosity;
use crate::core::{CliUnstable, Shell, SourceId, Workspace, WorkspaceRootConfig, features};
use crate::ops::RegistryCredentialConfig;
use crate::sources::CRATES_IO_INDEX;
use crate::sources::CRATES_IO_REGISTRY;
use crate::util::OnceExt as _;
use crate::util::errors::CargoResult;
use crate::util::network::http::configure_http_handle;
use crate::util::network::http::http_handle;
use crate::util::{CanonicalUrl, closest_msg, internal};
use crate::util::{Filesystem, IntoUrl, IntoUrlWithBase, Rustc};

use annotate_snippets::Level;
use anyhow::{Context as _, anyhow, bail, format_err};
use cargo_credential::Secret;
use cargo_util::paths;
use cargo_util_schemas::manifest::RegistryName;
use curl::easy::Easy;
use itertools::Itertools;
use serde::Deserialize;
use serde::de::IntoDeserializer as _;
use time::OffsetDateTime;
use toml_edit::Item;
use url::Url;

mod de;
use de::Deserializer;

mod error;
pub use error::ConfigError;

mod value;
pub use value::{Definition, OptValue, Value};

mod key;
pub use key::ConfigKey;

mod config_value;
pub use config_value::ConfigValue;
use config_value::is_nonmergeable_list;

mod path;
pub use path::{ConfigRelativePath, PathAndArgs};

mod target;
pub use target::{TargetCfgConfig, TargetConfig};

mod environment;
use environment::Env;

mod schema;
pub use schema::*;

use super::auth::RegistryConfig;

/// Helper macro for creating typed access methods.
macro_rules! get_value_typed {
    ($name:ident, $ty:ty, $variant:ident, $expected:expr) => {
        /// Low-level private method for getting a config value as an [`OptValue`].
        fn $name(&self, key: &ConfigKey) -> Result<OptValue<$ty>, ConfigError> {
            let cv = self.get_cv(key)?;
            let env = self.get_config_env::<$ty>(key)?;
            match (cv, env) {
                (Some(CV::$variant(val, definition)), Some(env)) => {
                    if definition.is_higher_priority(&env.definition) {
                        Ok(Some(Value { val, definition }))
                    } else {
                        Ok(Some(env))
                    }
                }
                (Some(CV::$variant(val, definition)), None) => Ok(Some(Value { val, definition })),
                (Some(cv), _) => Err(ConfigError::expected(key, $expected, &cv)),
                (None, Some(env)) => Ok(Some(env)),
                (None, None) => Ok(None),
            }
        }
    };
}

/// Indicates why a config value is being loaded.
#[derive(Clone, Copy, Debug)]
enum WhyLoad {
    /// Loaded due to a request from the global cli arg `--config`
    ///
    /// Indirect configs loaded via [`config-include`] are also seen as from cli args,
    /// if the initial config is being loaded from cli.
    ///
    /// [`config-include`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#config-include
    Cli,
    /// Loaded due to config file discovery.
    FileDiscovery,
}

/// A previously generated authentication token and the data needed to determine if it can be reused.
#[derive(Debug)]
pub struct CredentialCacheValue {
    pub token_value: Secret<String>,
    pub expiration: Option<OffsetDateTime>,
    pub operation_independent: bool,
}

/// Configuration information for cargo. This is not specific to a build, it is information
/// relating to cargo itself.
#[derive(Debug)]
pub struct GlobalContext {
    /// The location of the user's Cargo home directory. OS-dependent.
    home_path: Filesystem,
    /// Information about how to write messages to the shell
    shell: Mutex<Shell>,
    /// A collection of configuration options
    values: OnceLock<HashMap<String, ConfigValue>>,
    /// A collection of configuration options from the credentials file
    credential_values: OnceLock<HashMap<String, ConfigValue>>,
    /// CLI config values, passed in via `configure`.
    cli_config: Option<Vec<String>>,
    /// The current working directory of cargo
    cwd: PathBuf,
    /// Directory where config file searching should stop (inclusive).
    search_stop_path: Option<PathBuf>,
    /// The location of the cargo executable (path to current process)
    cargo_exe: OnceLock<PathBuf>,
    /// The location of the rustdoc executable
    rustdoc: OnceLock<PathBuf>,
    /// Whether we are printing extra verbose messages
    extra_verbose: bool,
    /// `frozen` is the same as `locked`, but additionally will not access the
    /// network to determine if the lock file is out-of-date.
    frozen: bool,
    /// `locked` is set if we should not update lock files. If the lock file
    /// is missing, or needs to be updated, an error is produced.
    locked: bool,
    /// `offline` is set if we should never access the network, but otherwise
    /// continue operating if possible.
    offline: bool,
    /// A global static IPC control mechanism (used for managing parallel builds)
    jobserver: Option<jobserver::Client>,
    /// Cli flags of the form "-Z something" merged with config file values
    unstable_flags: CliUnstable,
    /// Cli flags of the form "-Z something"
    unstable_flags_cli: Option<Vec<String>>,
    /// A handle on curl easy mode for http calls
    easy: OnceLock<Mutex<Easy>>,
    /// Cache of the `SourceId` for crates.io
    crates_io_source_id: OnceLock<SourceId>,
    /// If false, don't cache `rustc --version --verbose` invocations
    cache_rustc_info: bool,
    /// Creation time of this config, used to output the total build time
    creation_time: Instant,
    /// Target Directory via resolved Cli parameter
    target_dir: Option<Filesystem>,
    /// Environment variable snapshot.
    env: Env,
    /// Tracks which sources have been updated to avoid multiple updates.
    updated_sources: Mutex<HashSet<SourceId>>,
    /// Cache of credentials from configuration or credential providers.
    /// Maps from url to credential value.
    credential_cache: Mutex<HashMap<CanonicalUrl, CredentialCacheValue>>,
    /// Cache of registry config from the `[registries]` table.
    registry_config: Mutex<HashMap<SourceId, Option<RegistryConfig>>>,
    /// Locks on the package and index caches.
    package_cache_lock: CacheLocker,
    /// Cached configuration parsed by Cargo
    http_config: OnceLock<CargoHttpConfig>,
    future_incompat_config: OnceLock<CargoFutureIncompatConfig>,
    net_config: OnceLock<CargoNetConfig>,
    build_config: OnceLock<CargoBuildConfig>,
    target_cfgs: OnceLock<Vec<(String, TargetCfgConfig)>>,
    doc_extern_map: OnceLock<RustdocExternMap>,
    progress_config: ProgressConfig,
    env_config: OnceLock<Arc<HashMap<String, OsString>>>,
    /// This should be false if:
    /// - this is an artifact of the rustc distribution process for "stable" or for "beta"
    /// - this is an `#[test]` that does not opt in with `enable_nightly_features`
    /// - this is an integration test that uses `ProcessBuilder`
    ///      that does not opt in with `masquerade_as_nightly_cargo`
    /// This should be true if:
    /// - this is an artifact of the rustc distribution process for "nightly"
    /// - this is being used in the rustc distribution process internally
    /// - this is a cargo executable that was built from source
    /// - this is an `#[test]` that called `enable_nightly_features`
    /// - this is an integration test that uses `ProcessBuilder`
    ///       that called `masquerade_as_nightly_cargo`
    /// It's public to allow tests use nightly features.
    /// NOTE: this should be set before `configure()`. If calling this from an integration test,
    /// consider using `ConfigBuilder::enable_nightly_features` instead.
    pub nightly_features_allowed: bool,
    /// `WorkspaceRootConfigs` that have been found
    ws_roots: Mutex<HashMap<PathBuf, WorkspaceRootConfig>>,
    /// The global cache tracker is a database used to track disk cache usage.
    global_cache_tracker: OnceLock<Mutex<GlobalCacheTracker>>,
    /// A cache of modifications to make to [`GlobalContext::global_cache_tracker`],
    /// saved to disk in a batch to improve performance.
    deferred_global_last_use: OnceLock<Mutex<DeferredGlobalLastUse>>,
}

impl GlobalContext {
    /// Creates a new config instance.
    ///
    /// This is typically used for tests or other special cases. `default` is
    /// preferred otherwise.
    ///
    /// This does only minimal initialization. In particular, it does not load
    /// any config files from disk. Those will be loaded lazily as-needed.
    pub fn new(shell: Shell, cwd: PathBuf, homedir: PathBuf) -> GlobalContext {
        static mut GLOBAL_JOBSERVER: *mut jobserver::Client = 0 as *mut _;
        static INIT: Once = Once::new();

        // This should be called early on in the process, so in theory the
        // unsafety is ok here. (taken ownership of random fds)
        INIT.call_once(|| unsafe {
            if let Some(client) = jobserver::Client::from_env() {
                GLOBAL_JOBSERVER = Box::into_raw(Box::new(client));
            }
        });

        let env = Env::new();

        let cache_key = "CARGO_CACHE_RUSTC_INFO";
        let cache_rustc_info = match env.get_env_os(cache_key) {
            Some(cache) => cache != "0",
            _ => true,
        };

        GlobalContext {
            home_path: Filesystem::new(homedir),
            shell: Mutex::new(shell),
            cwd,
            search_stop_path: None,
            values: Default::default(),
            credential_values: Default::default(),
            cli_config: None,
            cargo_exe: Default::default(),
            rustdoc: Default::default(),
            extra_verbose: false,
            frozen: false,
            locked: false,
            offline: false,
            jobserver: unsafe {
                if GLOBAL_JOBSERVER.is_null() {
                    None
                } else {
                    Some((*GLOBAL_JOBSERVER).clone())
                }
            },
            unstable_flags: CliUnstable::default(),
            unstable_flags_cli: None,
            easy: Default::default(),
            crates_io_source_id: Default::default(),
            cache_rustc_info,
            creation_time: Instant::now(),
            target_dir: None,
            env,
            updated_sources: Default::default(),
            credential_cache: Default::default(),
            registry_config: Default::default(),
            package_cache_lock: CacheLocker::new(),
            http_config: Default::default(),
            future_incompat_config: Default::default(),
            net_config: Default::default(),
            build_config: Default::default(),
            target_cfgs: Default::default(),
            doc_extern_map: Default::default(),
            progress_config: ProgressConfig::default(),
            env_config: Default::default(),
            nightly_features_allowed: matches!(&*features::channel(), "nightly" | "dev"),
            ws_roots: Default::default(),
            global_cache_tracker: Default::default(),
            deferred_global_last_use: Default::default(),
        }
    }

    /// Creates a new instance, with all default settings.
    ///
    /// This does only minimal initialization. In particular, it does not load
    /// any config files from disk. Those will be loaded lazily as-needed.
    pub fn default() -> CargoResult<GlobalContext> {
        let shell = Shell::new();
        let cwd =
            env::current_dir().context("couldn't get the current directory of the process")?;
        let homedir = homedir(&cwd).ok_or_else(|| {
            anyhow!(
                "Cargo couldn't find your home directory. \
                 This probably means that $HOME was not set."
            )
        })?;
        Ok(GlobalContext::new(shell, cwd, homedir))
    }

    /// Gets the user's Cargo home directory (OS-dependent).
    pub fn home(&self) -> &Filesystem {
        &self.home_path
    }

    /// Returns a path to display to the user with the location of their home
    /// config file (to only be used for displaying a diagnostics suggestion,
    /// such as recommending where to add a config value).
    pub fn diagnostic_home_config(&self) -> String {
        let home = self.home_path.as_path_unlocked();
        let path = match self.get_file_path(home, "config", false) {
            Ok(Some(existing_path)) => existing_path,
            _ => home.join("config.toml"),
        };
        path.to_string_lossy().to_string()
    }

    /// Gets the Cargo Git directory (`<cargo_home>/git`).
    pub fn git_path(&self) -> Filesystem {
        self.home_path.join("git")
    }

    /// Gets the directory of code sources Cargo checkouts from Git bare repos
    /// (`<cargo_home>/git/checkouts`).
    pub fn git_checkouts_path(&self) -> Filesystem {
        self.git_path().join("checkouts")
    }

    /// Gets the directory for all Git bare repos Cargo clones
    /// (`<cargo_home>/git/db`).
    pub fn git_db_path(&self) -> Filesystem {
        self.git_path().join("db")
    }

    /// Gets the Cargo base directory for all registry information (`<cargo_home>/registry`).
    pub fn registry_base_path(&self) -> Filesystem {
        self.home_path.join("registry")
    }

    /// Gets the Cargo registry index directory (`<cargo_home>/registry/index`).
    pub fn registry_index_path(&self) -> Filesystem {
        self.registry_base_path().join("index")
    }

    /// Gets the Cargo registry cache directory (`<cargo_home>/registry/cache`).
    pub fn registry_cache_path(&self) -> Filesystem {
        self.registry_base_path().join("cache")
    }

    /// Gets the Cargo registry source directory (`<cargo_home>/registry/src`).
    pub fn registry_source_path(&self) -> Filesystem {
        self.registry_base_path().join("src")
    }

    /// Gets the default Cargo registry.
    pub fn default_registry(&self) -> CargoResult<Option<String>> {
        Ok(self
            .get_string("registry.default")?
            .map(|registry| registry.val))
    }

    /// Gets a reference to the shell, e.g., for writing error messages.
    pub fn shell(&self) -> MutexGuard<'_, Shell> {
        self.shell.lock().unwrap()
    }

    /// Assert [`Self::shell`] is not in use
    ///
    /// Testing might not identify bugs with two accesses to `shell` at once
    /// due to conditional logic,
    /// so place this outside of the conditions to catch these bugs in more situations.
    pub fn debug_assert_shell_not_borrowed(&self) {
        if cfg!(debug_assertions) {
            match self.shell.try_lock() {
                Ok(_) | Err(std::sync::TryLockError::Poisoned(_)) => (),
                Err(std::sync::TryLockError::WouldBlock) => panic!("shell is borrowed!"),
            }
        }
    }

    /// Gets the path to the `rustdoc` executable.
    pub fn rustdoc(&self) -> CargoResult<&Path> {
        self.rustdoc
            .try_borrow_with(|| Ok(self.get_tool(Tool::Rustdoc, &self.build_config()?.rustdoc)))
            .map(AsRef::as_ref)
    }

    /// Gets the path to the `rustc` executable.
    pub fn load_global_rustc(&self, ws: Option<&Workspace<'_>>) -> CargoResult<Rustc> {
        let cache_location =
            ws.map(|ws| ws.build_dir().join(".rustc_info.json").into_path_unlocked());
        let wrapper = self.maybe_get_tool("rustc_wrapper", &self.build_config()?.rustc_wrapper);
        let rustc_workspace_wrapper = self.maybe_get_tool(
            "rustc_workspace_wrapper",
            &self.build_config()?.rustc_workspace_wrapper,
        );

        Rustc::new(
            self.get_tool(Tool::Rustc, &self.build_config()?.rustc),
            wrapper,
            rustc_workspace_wrapper,
            &self
                .home()
                .join("bin")
                .join("rustc")
                .into_path_unlocked()
                .with_extension(env::consts::EXE_EXTENSION),
            if self.cache_rustc_info {
                cache_location
            } else {
                None
            },
            self,
        )
    }

    /// Gets the path to the `cargo` executable.
    pub fn cargo_exe(&self) -> CargoResult<&Path> {
        self.cargo_exe
            .try_borrow_with(|| {
                let from_env = || -> CargoResult<PathBuf> {
                    // Try re-using the `cargo` set in the environment already. This allows
                    // commands that use Cargo as a library to inherit (via `cargo <subcommand>`)
                    // or set (by setting `$CARGO`) a correct path to `cargo` when the current exe
                    // is not actually cargo (e.g., `cargo-*` binaries, Valgrind, `ld.so`, etc.).
                    let exe = self
                        .get_env_os(crate::CARGO_ENV)
                        .map(PathBuf::from)
                        .ok_or_else(|| anyhow!("$CARGO not set"))?;
                    Ok(exe)
                };

                fn from_current_exe() -> CargoResult<PathBuf> {
                    // Try fetching the path to `cargo` using `env::current_exe()`.
                    // The method varies per operating system and might fail; in particular,
                    // it depends on `/proc` being mounted on Linux, and some environments
                    // (like containers or chroots) may not have that available.
                    let exe = env::current_exe()?;
                    Ok(exe)
                }

                fn from_argv() -> CargoResult<PathBuf> {
                    // Grab `argv[0]` and attempt to resolve it to an absolute path.
                    // If `argv[0]` has one component, it must have come from a `PATH` lookup,
                    // so probe `PATH` in that case.
                    // Otherwise, it has multiple components and is either:
                    // - a relative path (e.g., `./cargo`, `target/debug/cargo`), or
                    // - an absolute path (e.g., `/usr/local/bin/cargo`).
                    let argv0 = env::args_os()
                        .map(PathBuf::from)
                        .next()
                        .ok_or_else(|| anyhow!("no argv[0]"))?;
                    paths::resolve_executable(&argv0)
                }

                // Determines whether `path` is a cargo binary.
                // See: https://github.com/rust-lang/cargo/issues/15099#issuecomment-2666737150
                fn is_cargo(path: &Path) -> bool {
                    path.file_stem() == Some(OsStr::new("cargo"))
                }

                let from_current_exe = from_current_exe();
                if from_current_exe.as_deref().is_ok_and(is_cargo) {
                    return from_current_exe;
                }

                let from_argv = from_argv();
                if from_argv.as_deref().is_ok_and(is_cargo) {
                    return from_argv;
                }

                let exe = from_env()
                    .or(from_current_exe)
                    .or(from_argv)
                    .context("couldn't get the path to cargo executable")?;
                Ok(exe)
            })
            .map(AsRef::as_ref)
    }

    /// Which package sources have been updated, used to ensure it is only done once.
    pub fn updated_sources(&self) -> MutexGuard<'_, HashSet<SourceId>> {
        self.updated_sources.lock().unwrap()
    }

    /// Cached credentials from credential providers or configuration.
    pub fn credential_cache(&self) -> MutexGuard<'_, HashMap<CanonicalUrl, CredentialCacheValue>> {
        self.credential_cache.lock().unwrap()
    }

    /// Cache of already parsed registries from the `[registries]` table.
    pub(crate) fn registry_config(
        &self,
    ) -> MutexGuard<'_, HashMap<SourceId, Option<RegistryConfig>>> {
        self.registry_config.lock().unwrap()
    }

    /// Gets all config values from disk.
    ///
    /// This will lazy-load the values as necessary. Callers are responsible
    /// for checking environment variables. Callers outside of the `config`
    /// module should avoid using this.
    pub fn values(&self) -> CargoResult<&HashMap<String, ConfigValue>> {
        self.values.try_borrow_with(|| self.load_values())
    }

    /// Gets a mutable copy of the on-disk config values.
    ///
    /// This requires the config values to already have been loaded. This
    /// currently only exists for `cargo vendor` to remove the `source`
    /// entries. This doesn't respect environment variables. You should avoid
    /// using this if possible.
    pub fn values_mut(&mut self) -> CargoResult<&mut HashMap<String, ConfigValue>> {
        let _ = self.values()?;
        Ok(self.values.get_mut().expect("already loaded config values"))
    }

    // Note: this is used by RLS, not Cargo.
    pub fn set_values(&self, values: HashMap<String, ConfigValue>) -> CargoResult<()> {
        if self.values.get().is_some() {
            bail!("config values already found")
        }
        match self.values.set(values.into()) {
            Ok(()) => Ok(()),
            Err(_) => bail!("could not fill values"),
        }
    }

    /// Sets the path where ancestor config file searching will stop. The
    /// given path is included, but its ancestors are not.
    pub fn set_search_stop_path<P: Into<PathBuf>>(&mut self, path: P) {
        let path = path.into();
        debug_assert!(self.cwd.starts_with(&path));
        self.search_stop_path = Some(path);
    }

    /// Switches the working directory to [`std::env::current_dir`]
    ///
    /// There is not a need to also call [`Self::reload_rooted_at`].
    pub fn reload_cwd(&mut self) -> CargoResult<()> {
        let cwd =
            env::current_dir().context("couldn't get the current directory of the process")?;
        let homedir = homedir(&cwd).ok_or_else(|| {
            anyhow!(
                "Cargo couldn't find your home directory. \
                 This probably means that $HOME was not set."
            )
        })?;

        self.cwd = cwd;
        self.home_path = Filesystem::new(homedir);
        self.reload_rooted_at(self.cwd.clone())?;
        Ok(())
    }

    /// Reloads on-disk configuration values, starting at the given path and
    /// walking up its ancestors.
    pub fn reload_rooted_at<P: AsRef<Path>>(&mut self, path: P) -> CargoResult<()> {
        let values = self.load_values_from(path.as_ref())?;
        self.values.replace(values);
        self.merge_cli_args()?;
        self.load_unstable_flags_from_config()?;
        Ok(())
    }

    /// The current working directory.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// The `target` output directory to use.
    ///
    /// Returns `None` if the user has not chosen an explicit directory.
    ///
    /// Callers should prefer [`Workspace::target_dir`] instead.
    pub fn target_dir(&self) -> CargoResult<Option<Filesystem>> {
        if let Some(dir) = &self.target_dir {
            Ok(Some(dir.clone()))
        } else if let Some(dir) = self.get_env_os("CARGO_TARGET_DIR") {
            // Check if the CARGO_TARGET_DIR environment variable is set to an empty string.
            if dir.is_empty() {
                bail!(
                    "the target directory is set to an empty string in the \
                     `CARGO_TARGET_DIR` environment variable"
                )
            }

            Ok(Some(Filesystem::new(self.cwd.join(dir))))
        } else if let Some(val) = &self.build_config()?.target_dir {
            let path = val.resolve_path(self);

            // Check if the target directory is set to an empty string in the config.toml file.
            if val.raw_value().is_empty() {
                bail!(
                    "the target directory is set to an empty string in {}",
                    val.value().definition
                )
            }

            Ok(Some(Filesystem::new(path)))
        } else {
            Ok(None)
        }
    }

    /// The directory to use for intermediate build artifacts.
    ///
    /// Callers should prefer [`Workspace::build_dir`] instead.
    pub fn build_dir(&self, workspace_manifest_path: &Path) -> CargoResult<Option<Filesystem>> {
        let Some(val) = &self.build_config()?.build_dir else {
            return Ok(None);
        };
        self.custom_build_dir(val, workspace_manifest_path)
            .map(Some)
    }

    /// The directory to use for intermediate build artifacts.
    ///
    /// Callers should prefer [`Workspace::build_dir`] instead.
    pub fn custom_build_dir(
        &self,
        val: &ConfigRelativePath,
        workspace_manifest_path: &Path,
    ) -> CargoResult<Filesystem> {
        let replacements = [
            (
                "{workspace-root}",
                workspace_manifest_path
                    .parent()
                    .unwrap()
                    .to_str()
                    .context("workspace root was not valid utf-8")?
                    .to_string(),
            ),
            (
                "{cargo-cache-home}",
                self.home()
                    .as_path_unlocked()
                    .to_str()
                    .context("cargo home was not valid utf-8")?
                    .to_string(),
            ),
            ("{workspace-path-hash}", {
                let real_path = std::fs::canonicalize(workspace_manifest_path)?;
                let hash = crate::util::hex::short_hash(&real_path);
                format!("{}{}{}", &hash[0..2], std::path::MAIN_SEPARATOR, &hash[2..])
            }),
        ];

        let template_variables = replacements
            .iter()
            .map(|(key, _)| key[1..key.len() - 1].to_string())
            .collect_vec();

        let path = val
                .resolve_templated_path(self, replacements)
                .map_err(|e| match e {
                    path::ResolveTemplateError::UnexpectedVariable {
                        variable,
                        raw_template,
                    } => {
                        let mut suggestion = closest_msg(&variable, template_variables.iter(), |key| key, "template variable");
                        if suggestion == "" {
                            let variables = template_variables.iter().map(|v| format!("`{{{v}}}`")).join(", ");
                            suggestion = format!("\n\nhelp: available template variables are {variables}");
                        }
                        anyhow!(
                            "unexpected variable `{variable}` in build.build-dir path `{raw_template}`{suggestion}"
                        )
                    },
                    path::ResolveTemplateError::UnexpectedBracket { bracket_type, raw_template } => {
                        let (btype, literal) = match bracket_type {
                            path::BracketType::Opening => ("opening", "{"),
                            path::BracketType::Closing => ("closing", "}"),
                        };

                        anyhow!(
                            "unexpected {btype} bracket `{literal}` in build.build-dir path `{raw_template}`"
                        )
                    }
                })?;

        // Check if the target directory is set to an empty string in the config.toml file.
        if val.raw_value().is_empty() {
            bail!(
                "the build directory is set to an empty string in {}",
                val.value().definition
            )
        }

        Ok(Filesystem::new(path))
    }

    /// Get a configuration value by key.
    ///
    /// This does NOT look at environment variables. See `get_cv_with_env` for
    /// a variant that supports environment variables.
    fn get_cv(&self, key: &ConfigKey) -> CargoResult<Option<ConfigValue>> {
        if let Some(vals) = self.credential_values.get() {
            let val = self.get_cv_helper(key, vals)?;
            if val.is_some() {
                return Ok(val);
            }
        }
        self.get_cv_helper(key, &*self.values()?)
    }

    fn get_cv_helper(
        &self,
        key: &ConfigKey,
        vals: &HashMap<String, ConfigValue>,
    ) -> CargoResult<Option<ConfigValue>> {
        tracing::trace!("get cv {:?}", key);
        if key.is_root() {
            // Returning the entire root table (for example `cargo config get`
            // with no key). The definition here shouldn't matter.
            return Ok(Some(CV::Table(
                vals.clone(),
                Definition::Path(PathBuf::new()),
            )));
        }
        let mut parts = key.parts().enumerate();
        let Some(mut val) = vals.get(parts.next().unwrap().1) else {
            return Ok(None);
        };
        for (i, part) in parts {
            match val {
                CV::Table(map, _) => {
                    val = match map.get(part) {
                        Some(val) => val,
                        None => return Ok(None),
                    }
                }
                CV::Integer(_, def)
                | CV::String(_, def)
                | CV::List(_, def)
                | CV::Boolean(_, def) => {
                    let mut key_so_far = ConfigKey::new();
                    for part in key.parts().take(i) {
                        key_so_far.push(part);
                    }
                    bail!(
                        "expected table for configuration key `{}`, \
                         but found {} in {}",
                        key_so_far,
                        val.desc(),
                        def
                    )
                }
            }
        }
        Ok(Some(val.clone()))
    }

    /// This is a helper for getting a CV from a file or env var.
    pub(crate) fn get_cv_with_env(&self, key: &ConfigKey) -> CargoResult<Option<CV>> {
        // Determine if value comes from env, cli, or file, and merge env if
        // possible.
        let cv = self.get_cv(key)?;
        if key.is_root() {
            // Root table can't have env value.
            return Ok(cv);
        }
        let env = self.env.get_str(key.as_env_key());
        let env_def = Definition::Environment(key.as_env_key().to_string());
        let use_env = match (&cv, env) {
            // Lists are always merged.
            (Some(CV::List(..)), Some(_)) => true,
            (Some(cv), Some(_)) => env_def.is_higher_priority(cv.definition()),
            (None, Some(_)) => true,
            _ => false,
        };

        if !use_env {
            return Ok(cv);
        }

        // Future note: If you ever need to deserialize a non-self describing
        // map type, this should implement a starts_with check (similar to how
        // ConfigMapAccess does).
        let env = env.unwrap();
        if env == "true" {
            Ok(Some(CV::Boolean(true, env_def)))
        } else if env == "false" {
            Ok(Some(CV::Boolean(false, env_def)))
        } else if let Ok(i) = env.parse::<i64>() {
            Ok(Some(CV::Integer(i, env_def)))
        } else if self.cli_unstable().advanced_env && env.starts_with('[') && env.ends_with(']') {
            match cv {
                Some(CV::List(mut cv_list, cv_def)) => {
                    // Merge with config file.
                    self.get_env_list(key, &mut cv_list)?;
                    Ok(Some(CV::List(cv_list, cv_def)))
                }
                Some(cv) => {
                    // This can't assume StringList.
                    // Return an error, which is the behavior of merging
                    // multiple config.toml files with the same scenario.
                    bail!(
                        "unable to merge array env for config `{}`\n\
                        file: {:?}\n\
                        env: {}",
                        key,
                        cv,
                        env
                    );
                }
                None => {
                    let mut cv_list = Vec::new();
                    self.get_env_list(key, &mut cv_list)?;
                    Ok(Some(CV::List(cv_list, env_def)))
                }
            }
        } else {
            // Try to merge if possible.
            match cv {
                Some(CV::List(mut cv_list, cv_def)) => {
                    // Merge with config file.
                    self.get_env_list(key, &mut cv_list)?;
                    Ok(Some(CV::List(cv_list, cv_def)))
                }
                _ => {
                    // Note: CV::Table merging is not implemented, as env
                    // vars do not support table values. In the future, we
                    // could check for `{}`, and interpret it as TOML if
                    // that seems useful.
                    Ok(Some(CV::String(env.to_string(), env_def)))
                }
            }
        }
    }

    /// Helper primarily for testing.
    pub fn set_env(&mut self, env: HashMap<String, String>) {
        self.env = Env::from_map(env);
    }

    /// Returns all environment variables as an iterator,
    /// keeping only entries where both the key and value are valid UTF-8.
    pub(crate) fn env(&self) -> impl Iterator<Item = (&str, &str)> {
        self.env.iter_str()
    }

    /// Returns all environment variable keys, filtering out keys that are not valid UTF-8.
    fn env_keys(&self) -> impl Iterator<Item = &str> {
        self.env.keys_str()
    }

    fn get_config_env<T>(&self, key: &ConfigKey) -> Result<OptValue<T>, ConfigError>
    where
        T: FromStr,
        <T as FromStr>::Err: fmt::Display,
    {
        match self.env.get_str(key.as_env_key()) {
            Some(value) => {
                let definition = Definition::Environment(key.as_env_key().to_string());
                Ok(Some(Value {
                    val: value
                        .parse()
                        .map_err(|e| ConfigError::new(format!("{}", e), definition.clone()))?,
                    definition,
                }))
            }
            None => {
                self.check_environment_key_case_mismatch(key);
                Ok(None)
            }
        }
    }

    /// Get the value of environment variable `key` through the snapshot in
    /// [`GlobalContext`].
    ///
    /// This can be used similarly to [`std::env::var`].
    pub fn get_env(&self, key: impl AsRef<OsStr>) -> CargoResult<&str> {
        self.env.get_env(key)
    }

    /// Get the value of environment variable `key` through the snapshot in
    /// [`GlobalContext`].
    ///
    /// This can be used similarly to [`std::env::var_os`].
    pub fn get_env_os(&self, key: impl AsRef<OsStr>) -> Option<&OsStr> {
        self.env.get_env_os(key)
    }

    /// Check if the [`GlobalContext`] contains a given [`ConfigKey`].
    ///
    /// See `ConfigMapAccess` for a description of `env_prefix_ok`.
    fn has_key(&self, key: &ConfigKey, env_prefix_ok: bool) -> CargoResult<bool> {
        if self.env.contains_key(key.as_env_key()) {
            return Ok(true);
        }
        if env_prefix_ok {
            let env_prefix = format!("{}_", key.as_env_key());
            if self.env_keys().any(|k| k.starts_with(&env_prefix)) {
                return Ok(true);
            }
        }
        if self.get_cv(key)?.is_some() {
            return Ok(true);
        }
        self.check_environment_key_case_mismatch(key);

        Ok(false)
    }

    fn check_environment_key_case_mismatch(&self, key: &ConfigKey) {
        if let Some(env_key) = self.env.get_normalized(key.as_env_key()) {
            let _ = self.shell().warn(format!(
                "environment variables are expected to use uppercase letters and underscores, \
                the variable `{}` will be ignored and have no effect",
                env_key
            ));
        }
    }

    /// Get a string config value.
    ///
    /// See `get` for more details.
    pub fn get_string(&self, key: &str) -> CargoResult<OptValue<String>> {
        self.get::<OptValue<String>>(key)
    }

    fn string_to_path(&self, value: &str, definition: &Definition) -> PathBuf {
        let is_path = value.contains('/') || (cfg!(windows) && value.contains('\\'));
        if is_path {
            definition.root(self.cwd()).join(value)
        } else {
            // A pathless name.
            PathBuf::from(value)
        }
    }

    /// Internal method for getting an environment variable as a list.
    /// If the key is a non-mergeable list and a value is found in the environment, existing values are cleared.
    fn get_env_list(&self, key: &ConfigKey, output: &mut Vec<ConfigValue>) -> CargoResult<()> {
        let Some(env_val) = self.env.get_str(key.as_env_key()) else {
            self.check_environment_key_case_mismatch(key);
            return Ok(());
        };

        let env_def = Definition::Environment(key.as_env_key().to_string());

        if is_nonmergeable_list(&key) {
            assert!(
                output
                    .windows(2)
                    .all(|cvs| cvs[0].definition() == cvs[1].definition()),
                "non-mergeable list must have only one definition: {output:?}",
            );

            // Keep existing config if higher priority than env (e.g., --config CLI),
            // otherwise clear for env
            if output
                .first()
                .map(|o| o.definition() > &env_def)
                .unwrap_or_default()
            {
                return Ok(());
            } else {
                output.clear();
            }
        }

        if self.cli_unstable().advanced_env && env_val.starts_with('[') && env_val.ends_with(']') {
            // Parse an environment string as a TOML array.
            let toml_v = env_val.parse::<toml::Value>().map_err(|e| {
                ConfigError::new(format!("could not parse TOML list: {}", e), env_def.clone())
            })?;
            let values = toml_v.as_array().expect("env var was not array");
            for value in values {
                // Until we figure out how to deal with it through `-Zadvanced-env`,
                // complex array types are unsupported.
                let s = value.as_str().ok_or_else(|| {
                    ConfigError::new(
                        format!("expected string, found {}", value.type_str()),
                        env_def.clone(),
                    )
                })?;
                output.push(CV::String(s.to_string(), env_def.clone()))
            }
        } else {
            output.extend(
                env_val
                    .split_whitespace()
                    .map(|s| CV::String(s.to_string(), env_def.clone())),
            );
        }
        output.sort_by(|a, b| a.definition().cmp(b.definition()));
        Ok(())
    }

    /// Low-level method for getting a config value as an `OptValue<HashMap<String, CV>>`.
    ///
    /// NOTE: This does not read from env. The caller is responsible for that.
    fn get_table(&self, key: &ConfigKey) -> CargoResult<OptValue<HashMap<String, CV>>> {
        match self.get_cv(key)? {
            Some(CV::Table(val, definition)) => Ok(Some(Value { val, definition })),
            Some(val) => self.expected("table", key, &val),
            None => Ok(None),
        }
    }

    get_value_typed! {get_integer, i64, Integer, "an integer"}
    get_value_typed! {get_bool, bool, Boolean, "true/false"}
    get_value_typed! {get_string_priv, String, String, "a string"}

    /// Generate an error when the given value is the wrong type.
    fn expected<T>(&self, ty: &str, key: &ConfigKey, val: &CV) -> CargoResult<T> {
        val.expected(ty, &key.to_string())
            .map_err(|e| anyhow!("invalid configuration for key `{}`\n{}", key, e))
    }

    /// Update the instance based on settings typically passed in on
    /// the command-line.
    ///
    /// This may also load the config from disk if it hasn't already been
    /// loaded.
    pub fn configure(
        &mut self,
        verbose: u32,
        quiet: bool,
        color: Option<&str>,
        frozen: bool,
        locked: bool,
        offline: bool,
        target_dir: &Option<PathBuf>,
        unstable_flags: &[String],
        cli_config: &[String],
    ) -> CargoResult<()> {
        for warning in self
            .unstable_flags
            .parse(unstable_flags, self.nightly_features_allowed)?
        {
            self.shell().warn(warning)?;
        }
        if !unstable_flags.is_empty() {
            // store a copy of the cli flags separately for `load_unstable_flags_from_config`
            // (we might also need it again for `reload_rooted_at`)
            self.unstable_flags_cli = Some(unstable_flags.to_vec());
        }
        if !cli_config.is_empty() {
            self.cli_config = Some(cli_config.iter().map(|s| s.to_string()).collect());
            self.merge_cli_args()?;
        }

        // Load the unstable flags from config file here first, as the config
        // file itself may enable inclusion of other configs. In that case, we
        // want to re-load configs with includes enabled:
        self.load_unstable_flags_from_config()?;
        if self.unstable_flags.config_include {
            // If the config was already loaded (like when fetching the
            // `[alias]` table), it was loaded with includes disabled because
            // the `unstable_flags` hadn't been set up, yet. Any values
            // fetched before this step will not process includes, but that
            // should be fine (`[alias]` is one of the only things loaded
            // before configure). This can be removed when stabilized.
            self.reload_rooted_at(self.cwd.clone())?;
        }

        // Ignore errors in the configuration files. We don't want basic
        // commands like `cargo version` to error out due to config file
        // problems.
        let term = self.get::<TermConfig>("term").unwrap_or_default();

        // The command line takes precedence over configuration.
        let extra_verbose = verbose >= 2;
        let verbose = verbose != 0;
        let verbosity = match (verbose, quiet) {
            (true, true) => bail!("cannot set both --verbose and --quiet"),
            (true, false) => Verbosity::Verbose,
            (false, true) => Verbosity::Quiet,
            (false, false) => match (term.verbose, term.quiet) {
                (Some(true), Some(true)) => {
                    bail!("cannot set both `term.verbose` and `term.quiet`")
                }
                (Some(true), _) => Verbosity::Verbose,
                (_, Some(true)) => Verbosity::Quiet,
                _ => Verbosity::Normal,
            },
        };
        self.shell().set_verbosity(verbosity);
        self.extra_verbose = extra_verbose;

        let color = color.or_else(|| term.color.as_deref());
        self.shell().set_color_choice(color)?;
        if let Some(hyperlinks) = term.hyperlinks {
            self.shell().set_hyperlinks(hyperlinks)?;
        }
        if let Some(unicode) = term.unicode {
            self.shell().set_unicode(unicode)?;
        }

        self.progress_config = term.progress.unwrap_or_default();

        self.frozen = frozen;
        self.locked = locked;
        self.offline = offline
            || self
                .net_config()
                .ok()
                .and_then(|n| n.offline)
                .unwrap_or(false);
        let cli_target_dir = target_dir.as_ref().map(|dir| Filesystem::new(dir.clone()));
        self.target_dir = cli_target_dir;

        Ok(())
    }

    fn load_unstable_flags_from_config(&mut self) -> CargoResult<()> {
        // If nightly features are enabled, allow setting Z-flags from config
        // using the `unstable` table. Ignore that block otherwise.
        if self.nightly_features_allowed {
            self.unstable_flags = self
                .get::<Option<CliUnstable>>("unstable")?
                .unwrap_or_default();
            if let Some(unstable_flags_cli) = &self.unstable_flags_cli {
                // NB. It's not ideal to parse these twice, but doing it again here
                //     allows the CLI to override config files for both enabling
                //     and disabling, and doing it up top allows CLI Zflags to
                //     control config parsing behavior.
                self.unstable_flags.parse(unstable_flags_cli, true)?;
            }
        }

        Ok(())
    }

    pub fn cli_unstable(&self) -> &CliUnstable {
        &self.unstable_flags
    }

    pub fn extra_verbose(&self) -> bool {
        self.extra_verbose
    }

    pub fn network_allowed(&self) -> bool {
        !self.offline_flag().is_some()
    }

    pub fn offline_flag(&self) -> Option<&'static str> {
        if self.frozen {
            Some("--frozen")
        } else if self.offline {
            Some("--offline")
        } else {
            None
        }
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn lock_update_allowed(&self) -> bool {
        !self.locked_flag().is_some()
    }

    pub fn locked_flag(&self) -> Option<&'static str> {
        if self.frozen {
            Some("--frozen")
        } else if self.locked {
            Some("--locked")
        } else {
            None
        }
    }

    /// Loads configuration from the filesystem.
    pub fn load_values(&self) -> CargoResult<HashMap<String, ConfigValue>> {
        self.load_values_from(&self.cwd)
    }

    /// Like [`load_values`](GlobalContext::load_values) but without merging config values.
    ///
    /// This is primarily crafted for `cargo config` command.
    pub(crate) fn load_values_unmerged(&self) -> CargoResult<Vec<ConfigValue>> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let home = self.home_path.clone().into_path_unlocked();
        self.walk_tree(&self.cwd, &home, |path| {
            let mut cv = self._load_file(path, &mut seen, false, WhyLoad::FileDiscovery)?;
            if self.cli_unstable().config_include {
                self.load_unmerged_include(&mut cv, &mut seen, &mut result)?;
            }
            result.push(cv);
            Ok(())
        })
        .context("could not load Cargo configuration")?;
        Ok(result)
    }

    /// Like [`load_includes`](GlobalContext::load_includes) but without merging config values.
    ///
    /// This is primarily crafted for `cargo config` command.
    fn load_unmerged_include(
        &self,
        cv: &mut CV,
        seen: &mut HashSet<PathBuf>,
        output: &mut Vec<CV>,
    ) -> CargoResult<()> {
        let includes = self.include_paths(cv, false)?;
        for include in includes {
            let Some(abs_path) = include.resolve_path(self) else {
                continue;
            };

            let mut cv = self
                ._load_file(&abs_path, seen, false, WhyLoad::FileDiscovery)
                .with_context(|| {
                    format!(
                        "failed to load config include `{}` from `{}`",
                        include.path.display(),
                        include.def
                    )
                })?;
            self.load_unmerged_include(&mut cv, seen, output)?;
            output.push(cv);
        }
        Ok(())
    }

    /// Start a config file discovery from a path and merges all config values found.
    fn load_values_from(&self, path: &Path) -> CargoResult<HashMap<String, ConfigValue>> {
        // The root config value container isn't from any external source,
        // so its definition should be built-in.
        let mut cfg = CV::Table(HashMap::new(), Definition::BuiltIn);
        let home = self.home_path.clone().into_path_unlocked();

        self.walk_tree(path, &home, |path| {
            let value = self.load_file(path)?;
            cfg.merge(value, false).with_context(|| {
                format!("failed to merge configuration at `{}`", path.display())
            })?;
            Ok(())
        })
        .context("could not load Cargo configuration")?;

        match cfg {
            CV::Table(map, _) => Ok(map),
            _ => unreachable!(),
        }
    }

    /// Loads a config value from a path.
    ///
    /// This is used during config file discovery.
    fn load_file(&self, path: &Path) -> CargoResult<ConfigValue> {
        self._load_file(path, &mut HashSet::new(), true, WhyLoad::FileDiscovery)
    }

    /// Loads a config value from a path with options.
    ///
    /// This is actual implementation of loading a config value from a path.
    ///
    /// * `includes` determines whether to load configs from [`config-include`].
    /// * `seen` is used to check for cyclic includes.
    /// * `why_load` tells why a config is being loaded.
    ///
    /// [`config-include`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#config-include
    fn _load_file(
        &self,
        path: &Path,
        seen: &mut HashSet<PathBuf>,
        includes: bool,
        why_load: WhyLoad,
    ) -> CargoResult<ConfigValue> {
        if !seen.insert(path.to_path_buf()) {
            bail!(
                "config `include` cycle detected with path `{}`",
                path.display()
            );
        }
        tracing::debug!(?path, ?why_load, includes, "load config from file");

        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read configuration file `{}`", path.display()))?;
        let toml = parse_document(&contents, path, self).with_context(|| {
            format!("could not parse TOML configuration in `{}`", path.display())
        })?;
        let def = match why_load {
            WhyLoad::Cli => Definition::Cli(Some(path.into())),
            WhyLoad::FileDiscovery => Definition::Path(path.into()),
        };
        let value = CV::from_toml(def, toml::Value::Table(toml)).with_context(|| {
            format!(
                "failed to load TOML configuration from `{}`",
                path.display()
            )
        })?;
        if includes {
            self.load_includes(value, seen, why_load)
        } else {
            Ok(value)
        }
    }

    /// Load any `include` files listed in the given `value`.
    ///
    /// Returns `value` with the given include files merged into it.
    ///
    /// * `seen` is used to check for cyclic includes.
    /// * `why_load` tells why a config is being loaded.
    fn load_includes(
        &self,
        mut value: CV,
        seen: &mut HashSet<PathBuf>,
        why_load: WhyLoad,
    ) -> CargoResult<CV> {
        // Get the list of files to load.
        let includes = self.include_paths(&mut value, true)?;
        // Check unstable.
        if !self.cli_unstable().config_include {
            return Ok(value);
        }
        // Accumulate all values here.
        let mut root = CV::Table(HashMap::new(), value.definition().clone());
        for include in includes {
            let Some(abs_path) = include.resolve_path(self) else {
                continue;
            };

            self._load_file(&abs_path, seen, true, why_load)
                .and_then(|include| root.merge(include, true))
                .with_context(|| {
                    format!(
                        "failed to load config include `{}` from `{}`",
                        include.path.display(),
                        include.def
                    )
                })?;
        }
        root.merge(value, true)?;
        Ok(root)
    }

    /// Converts the `include` config value to a list of absolute paths.
    fn include_paths(&self, cv: &mut CV, remove: bool) -> CargoResult<Vec<ConfigInclude>> {
        let CV::Table(table, _def) = cv else {
            unreachable!()
        };
        let include = if remove {
            table.remove("include").map(Cow::Owned)
        } else {
            table.get("include").map(Cow::Borrowed)
        };
        let includes = match include.map(|c| c.into_owned()) {
            Some(CV::String(s, def)) => vec![ConfigInclude::new(s, def)],
            Some(CV::List(list, _def)) => list
                .into_iter()
                .enumerate()
                .map(|(idx, cv)| match cv {
                    CV::String(s, def) => Ok(ConfigInclude::new(s, def)),
                    CV::Table(mut table, def) => {
                        // Extract `include.path`
                        let s = match table.remove("path") {
                            Some(CV::String(s, _)) => s,
                            Some(other) => bail!(
                                "expected a string, but found {} at `include[{idx}].path` in `{def}`",
                                other.desc()
                            ),
                            None => bail!("missing field `path` at `include[{idx}]` in `{def}`"),
                        };

                        // Extract optional `include.optional` field
                        let optional = match table.remove("optional") {
                            Some(CV::Boolean(b, _)) => b,
                            Some(other) => bail!(
                                "expected a boolean, but found {} at `include[{idx}].optional` in `{def}`",
                                other.desc()
                            ),
                            None => false,
                        };

                        let mut include = ConfigInclude::new(s, def);
                        include.optional = optional;
                        Ok(include)
                    }
                    other => bail!(
                        "expected a string or table, but found {} at `include[{idx}]` in {}",
                        other.desc(),
                        other.definition(),
                    ),
                })
                .collect::<CargoResult<Vec<_>>>()?,
            Some(other) => bail!(
                "expected a string or list of strings, but found {} at `include` in `{}",
                other.desc(),
                other.definition()
            ),
            None => {
                return Ok(Vec::new());
            }
        };

        for include in &includes {
            if include.path.extension() != Some(OsStr::new("toml")) {
                bail!(
                    "expected a config include path ending with `.toml`, \
                     but found `{}` from `{}`",
                    include.path.display(),
                    include.def,
                )
            }
        }

        Ok(includes)
    }

    /// Parses the CLI config args and returns them as a table.
    pub(crate) fn cli_args_as_table(&self) -> CargoResult<ConfigValue> {
        let mut loaded_args = CV::Table(HashMap::new(), Definition::Cli(None));
        let Some(cli_args) = &self.cli_config else {
            return Ok(loaded_args);
        };
        let mut seen = HashSet::new();
        for arg in cli_args {
            let arg_as_path = self.cwd.join(arg);
            let tmp_table = if !arg.is_empty() && arg_as_path.exists() {
                // --config path_to_file
                self._load_file(&arg_as_path, &mut seen, true, WhyLoad::Cli)
                    .with_context(|| {
                        format!("failed to load config from `{}`", arg_as_path.display())
                    })?
            } else {
                let doc = toml_dotted_keys(arg)?;
                let doc: toml::Value = toml::Value::deserialize(doc.into_deserializer())
                    .with_context(|| {
                        format!("failed to parse value from --config argument `{arg}`")
                    })?;

                if doc
                    .get("registry")
                    .and_then(|v| v.as_table())
                    .and_then(|t| t.get("token"))
                    .is_some()
                {
                    bail!("registry.token cannot be set through --config for security reasons");
                } else if let Some((k, _)) = doc
                    .get("registries")
                    .and_then(|v| v.as_table())
                    .and_then(|t| t.iter().find(|(_, v)| v.get("token").is_some()))
                {
                    bail!(
                        "registries.{}.token cannot be set through --config for security reasons",
                        k
                    );
                }

                if doc
                    .get("registry")
                    .and_then(|v| v.as_table())
                    .and_then(|t| t.get("secret-key"))
                    .is_some()
                {
                    bail!(
                        "registry.secret-key cannot be set through --config for security reasons"
                    );
                } else if let Some((k, _)) = doc
                    .get("registries")
                    .and_then(|v| v.as_table())
                    .and_then(|t| t.iter().find(|(_, v)| v.get("secret-key").is_some()))
                {
                    bail!(
                        "registries.{}.secret-key cannot be set through --config for security reasons",
                        k
                    );
                }

                CV::from_toml(Definition::Cli(None), doc)
                    .with_context(|| format!("failed to convert --config argument `{arg}`"))?
            };
            let tmp_table = self
                .load_includes(tmp_table, &mut HashSet::new(), WhyLoad::Cli)
                .context("failed to load --config include".to_string())?;
            loaded_args
                .merge(tmp_table, true)
                .with_context(|| format!("failed to merge --config argument `{arg}`"))?;
        }
        Ok(loaded_args)
    }

    /// Add config arguments passed on the command line.
    fn merge_cli_args(&mut self) -> CargoResult<()> {
        let cv_from_cli = self.cli_args_as_table()?;
        assert!(cv_from_cli.is_table(), "cv from CLI must be a table");

        let root_cv = mem::take(self.values_mut()?);
        // The root config value container isn't from any external source,
        // so its definition should be built-in.
        let mut root_cv = CV::Table(root_cv, Definition::BuiltIn);
        root_cv.merge(cv_from_cli, true)?;

        // Put it back to gctx
        mem::swap(self.values_mut()?, root_cv.table_mut("<root>")?.0);

        Ok(())
    }

    /// The purpose of this function is to aid in the transition to using
    /// .toml extensions on Cargo's config files, which were historically not used.
    /// Both 'config.toml' and 'credentials.toml' should be valid with or without extension.
    /// When both exist, we want to prefer the one without an extension for
    /// backwards compatibility, but warn the user appropriately.
    fn get_file_path(
        &self,
        dir: &Path,
        filename_without_extension: &str,
        warn: bool,
    ) -> CargoResult<Option<PathBuf>> {
        let possible = dir.join(filename_without_extension);
        let possible_with_extension = dir.join(format!("{}.toml", filename_without_extension));

        if let Ok(possible_handle) = same_file::Handle::from_path(&possible) {
            if warn {
                if let Ok(possible_with_extension_handle) =
                    same_file::Handle::from_path(&possible_with_extension)
                {
                    // We don't want to print a warning if the version
                    // without the extension is just a symlink to the version
                    // WITH an extension, which people may want to do to
                    // support multiple Cargo versions at once and not
                    // get a warning.
                    if possible_handle != possible_with_extension_handle {
                        self.shell().warn(format!(
                            "both `{}` and `{}` exist. Using `{}`",
                            possible.display(),
                            possible_with_extension.display(),
                            possible.display()
                        ))?;
                    }
                } else {
                    self.shell().print_report(&[
                        Level::WARNING.secondary_title(
                        format!(
                        "`{}` is deprecated in favor of `{filename_without_extension}.toml`",
                        possible.display(),
                    )).element(Level::HELP.message(
                        format!("if you need to support cargo 1.38 or earlier, you can symlink `{filename_without_extension}` to `{filename_without_extension}.toml`")))

                    ], false)?;
                }
            }

            Ok(Some(possible))
        } else if possible_with_extension.exists() {
            Ok(Some(possible_with_extension))
        } else {
            Ok(None)
        }
    }

    fn walk_tree<F>(&self, pwd: &Path, home: &Path, mut walk: F) -> CargoResult<()>
    where
        F: FnMut(&Path) -> CargoResult<()>,
    {
        let mut seen_dir = HashSet::new();

        for current in paths::ancestors(pwd, self.search_stop_path.as_deref()) {
            let config_root = current.join(".cargo");
            if let Some(path) = self.get_file_path(&config_root, "config", true)? {
                walk(&path)?;
            }
            seen_dir.insert(config_root);
        }

        // Once we're done, also be sure to walk the home directory even if it's not
        // in our history to be sure we pick up that standard location for
        // information.
        if !seen_dir.contains(home) {
            if let Some(path) = self.get_file_path(home, "config", true)? {
                walk(&path)?;
            }
        }

        Ok(())
    }

    /// Gets the index for a registry.
    pub fn get_registry_index(&self, registry: &str) -> CargoResult<Url> {
        RegistryName::new(registry)?;
        if let Some(index) = self.get_string(&format!("registries.{}.index", registry))? {
            self.resolve_registry_index(&index).with_context(|| {
                format!(
                    "invalid index URL for registry `{}` defined in {}",
                    registry, index.definition
                )
            })
        } else {
            bail!(
                "registry index was not found in any configuration: `{}`",
                registry
            );
        }
    }

    /// Returns an error if `registry.index` is set.
    pub fn check_registry_index_not_set(&self) -> CargoResult<()> {
        if self.get_string("registry.index")?.is_some() {
            bail!(
                "the `registry.index` config value is no longer supported\n\
                Use `[source]` replacement to alter the default index for crates.io."
            );
        }
        Ok(())
    }

    fn resolve_registry_index(&self, index: &Value<String>) -> CargoResult<Url> {
        // This handles relative file: URLs, relative to the config definition.
        let base = index
            .definition
            .root(self.cwd())
            .join("truncated-by-url_with_base");
        // Parse val to check it is a URL, not a relative path without a protocol.
        let _parsed = index.val.into_url()?;
        let url = index.val.into_url_with_base(Some(&*base))?;
        if url.password().is_some() {
            bail!("registry URLs may not contain passwords");
        }
        Ok(url)
    }

    /// Loads credentials config from the credentials file, if present.
    ///
    /// The credentials are loaded into a separate field to enable them
    /// to be lazy-loaded after the main configuration has been loaded,
    /// without requiring `mut` access to the [`GlobalContext`].
    ///
    /// If the credentials are already loaded, this function does nothing.
    pub fn load_credentials(&self) -> CargoResult<()> {
        if self.credential_values.filled() {
            return Ok(());
        }

        let home_path = self.home_path.clone().into_path_unlocked();
        let Some(credentials) = self.get_file_path(&home_path, "credentials", true)? else {
            return Ok(());
        };

        let mut value = self.load_file(&credentials)?;
        // Backwards compatibility for old `.cargo/credentials` layout.
        {
            let (value_map, def) = value.table_mut("<root>")?;

            if let Some(token) = value_map.remove("token") {
                value_map.entry("registry".into()).or_insert_with(|| {
                    let map = HashMap::from([("token".into(), token)]);
                    CV::Table(map, def.clone())
                });
            }
        }

        let mut credential_values = HashMap::new();
        if let CV::Table(map, _) = value {
            let base_map = self.values()?;
            for (k, v) in map {
                let entry = match base_map.get(&k) {
                    Some(base_entry) => {
                        let mut entry = base_entry.clone();
                        entry.merge(v, true)?;
                        entry
                    }
                    None => v,
                };
                credential_values.insert(k, entry);
            }
        }
        self.credential_values
            .set(credential_values)
            .expect("was not filled at beginning of the function");
        Ok(())
    }

    /// Looks for a path for `tool` in an environment variable or the given config, and returns
    /// `None` if it's not present.
    fn maybe_get_tool(
        &self,
        tool: &str,
        from_config: &Option<ConfigRelativePath>,
    ) -> Option<PathBuf> {
        let var = tool.to_uppercase();

        match self.get_env_os(&var).as_ref().and_then(|s| s.to_str()) {
            Some(tool_path) => {
                let maybe_relative = tool_path.contains('/') || tool_path.contains('\\');
                let path = if maybe_relative {
                    self.cwd.join(tool_path)
                } else {
                    PathBuf::from(tool_path)
                };
                Some(path)
            }

            None => from_config.as_ref().map(|p| p.resolve_program(self)),
        }
    }

    /// Returns the path for the given tool.
    ///
    /// This will look for the tool in the following order:
    ///
    /// 1. From an environment variable matching the tool name (such as `RUSTC`).
    /// 2. From the given config value (which is usually something like `build.rustc`).
    /// 3. Finds the tool in the PATH environment variable.
    ///
    /// This is intended for tools that are rustup proxies. If you need to get
    /// a tool that is not a rustup proxy, use `maybe_get_tool` instead.
    fn get_tool(&self, tool: Tool, from_config: &Option<ConfigRelativePath>) -> PathBuf {
        let tool_str = tool.as_str();
        self.maybe_get_tool(tool_str, from_config)
            .or_else(|| {
                // This is an optimization to circumvent the rustup proxies
                // which can have a significant performance hit. The goal here
                // is to determine if calling `rustc` from PATH would end up
                // calling the proxies.
                //
                // This is somewhat cautious trying to determine if it is safe
                // to circumvent rustup, because there are some situations
                // where users may do things like modify PATH, call cargo
                // directly, use a custom rustup toolchain link without a
                // cargo executable, etc. However, there is still some risk
                // this may make the wrong decision in unusual circumstances.
                //
                // First, we must be running under rustup in the first place.
                let toolchain = self.get_env_os("RUSTUP_TOOLCHAIN")?;
                // This currently does not support toolchain paths.
                // This also enforces UTF-8.
                if toolchain.to_str()?.contains(&['/', '\\']) {
                    return None;
                }
                // If the tool on PATH is the same as `rustup` on path, then
                // there is pretty good evidence that it will be a proxy.
                let tool_resolved = paths::resolve_executable(Path::new(tool_str)).ok()?;
                let rustup_resolved = paths::resolve_executable(Path::new("rustup")).ok()?;
                let tool_meta = tool_resolved.metadata().ok()?;
                let rustup_meta = rustup_resolved.metadata().ok()?;
                // This works on the assumption that rustup and its proxies
                // use hard links to a single binary. If rustup ever changes
                // that setup, then I think the worst consequence is that this
                // optimization will not work, and it will take the slow path.
                if tool_meta.len() != rustup_meta.len() {
                    return None;
                }
                // Try to find the tool in rustup's toolchain directory.
                let tool_exe = Path::new(tool_str).with_extension(env::consts::EXE_EXTENSION);
                let toolchain_exe = home::rustup_home()
                    .ok()?
                    .join("toolchains")
                    .join(&toolchain)
                    .join("bin")
                    .join(&tool_exe);
                toolchain_exe.exists().then_some(toolchain_exe)
            })
            .unwrap_or_else(|| PathBuf::from(tool_str))
    }

    /// Get the `paths` overrides config value.
    pub fn paths_overrides(&self) -> CargoResult<OptValue<Vec<(String, Definition)>>> {
        let key = ConfigKey::from_str("paths");
        // paths overrides cannot be set via env config, so use get_cv here.
        match self.get_cv(&key)? {
            Some(CV::List(val, definition)) => {
                let val = val
                    .into_iter()
                    .map(|cv| match cv {
                        CV::String(s, def) => Ok((s, def)),
                        other => self.expected("string", &key, &other),
                    })
                    .collect::<CargoResult<Vec<_>>>()?;
                Ok(Some(Value { val, definition }))
            }
            Some(val) => self.expected("list", &key, &val),
            None => Ok(None),
        }
    }

    pub fn jobserver_from_env(&self) -> Option<&jobserver::Client> {
        self.jobserver.as_ref()
    }

    pub fn http(&self) -> CargoResult<&Mutex<Easy>> {
        let http = self
            .easy
            .try_borrow_with(|| http_handle(self).map(Into::into))?;
        {
            let mut http = http.lock().unwrap();
            http.reset();
            let timeout = configure_http_handle(self, &mut http)?;
            timeout.configure(&mut http)?;
        }
        Ok(http)
    }

    pub fn http_config(&self) -> CargoResult<&CargoHttpConfig> {
        self.http_config.try_borrow_with(|| {
            let mut http = self.get::<CargoHttpConfig>("http")?;
            let curl_v = curl::Version::get();
            disables_multiplexing_for_bad_curl(curl_v.version(), &mut http, self);
            Ok(http)
        })
    }

    pub fn future_incompat_config(&self) -> CargoResult<&CargoFutureIncompatConfig> {
        self.future_incompat_config
            .try_borrow_with(|| self.get::<CargoFutureIncompatConfig>("future-incompat-report"))
    }

    pub fn net_config(&self) -> CargoResult<&CargoNetConfig> {
        self.net_config
            .try_borrow_with(|| self.get::<CargoNetConfig>("net"))
    }

    pub fn build_config(&self) -> CargoResult<&CargoBuildConfig> {
        self.build_config
            .try_borrow_with(|| self.get::<CargoBuildConfig>("build"))
    }

    pub fn progress_config(&self) -> &ProgressConfig {
        &self.progress_config
    }

    /// Get the env vars from the config `[env]` table which
    /// are `force = true` or don't exist in the env snapshot [`GlobalContext::get_env`].
    pub fn env_config(&self) -> CargoResult<&Arc<HashMap<String, OsString>>> {
        let env_config = self.env_config.try_borrow_with(|| {
            CargoResult::Ok(Arc::new({
                let env_config = self.get::<EnvConfig>("env")?;
                // Reasons for disallowing these values:
                //
                // - CARGO_HOME: The initial call to cargo does not honor this value
                //   from the [env] table. Recursive calls to cargo would use the new
                //   value, possibly behaving differently from the outer cargo.
                //
                // - RUSTUP_HOME and RUSTUP_TOOLCHAIN: Under normal usage with rustup,
                //   this will have no effect because the rustup proxy sets
                //   RUSTUP_HOME and RUSTUP_TOOLCHAIN, and that would override the
                //   [env] table. If the outer cargo is executed directly
                //   circumventing the rustup proxy, then this would affect calls to
                //   rustc (assuming that is a proxy), which could potentially cause
                //   problems with cargo and rustc being from different toolchains. We
                //   consider this to be not a use case we would like to support,
                //   since it will likely cause problems or lead to confusion.
                for disallowed in &["CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"] {
                    if env_config.contains_key(*disallowed) {
                        bail!(
                            "setting the `{disallowed}` environment variable is not supported \
                            in the `[env]` configuration table"
                        );
                    }
                }
                env_config
                    .into_iter()
                    .filter_map(|(k, v)| {
                        if v.is_force() || self.get_env_os(&k).is_none() {
                            Some((k, v.resolve(self.cwd()).to_os_string()))
                        } else {
                            None
                        }
                    })
                    .collect()
            }))
        })?;

        Ok(env_config)
    }

    /// This is used to validate the `term` table has valid syntax.
    ///
    /// This is necessary because loading the term settings happens very
    /// early, and in some situations (like `cargo version`) we don't want to
    /// fail if there are problems with the config file.
    pub fn validate_term_config(&self) -> CargoResult<()> {
        drop(self.get::<TermConfig>("term")?);
        Ok(())
    }

    /// Returns a list of `target.'cfg()'` tables.
    ///
    /// The list is sorted by the table name.
    pub fn target_cfgs(&self) -> CargoResult<&Vec<(String, TargetCfgConfig)>> {
        self.target_cfgs
            .try_borrow_with(|| target::load_target_cfgs(self))
    }

    pub fn doc_extern_map(&self) -> CargoResult<&RustdocExternMap> {
        // Note: This does not support environment variables. The `Unit`
        // fundamentally does not have access to the registry name, so there is
        // nothing to query. Plumbing the name into SourceId is quite challenging.
        self.doc_extern_map
            .try_borrow_with(|| self.get::<RustdocExternMap>("doc.extern-map"))
    }

    /// Returns true if the `[target]` table should be applied to host targets.
    pub fn target_applies_to_host(&self) -> CargoResult<bool> {
        target::get_target_applies_to_host(self)
    }

    /// Returns the `[host]` table definition for the given target triple.
    pub fn host_cfg_triple(&self, target: &str) -> CargoResult<TargetConfig> {
        target::load_host_triple(self, target)
    }

    /// Returns the `[target]` table definition for the given target triple.
    pub fn target_cfg_triple(&self, target: &str) -> CargoResult<TargetConfig> {
        target::load_target_triple(self, target)
    }

    /// Returns the cached [`SourceId`] corresponding to the main repository.
    ///
    /// This is the main cargo registry by default, but it can be overridden in
    /// a `.cargo/config.toml`.
    pub fn crates_io_source_id(&self) -> CargoResult<SourceId> {
        let source_id = self.crates_io_source_id.try_borrow_with(|| {
            self.check_registry_index_not_set()?;
            let url = CRATES_IO_INDEX.into_url().unwrap();
            SourceId::for_alt_registry(&url, CRATES_IO_REGISTRY)
        })?;
        Ok(*source_id)
    }

    pub fn creation_time(&self) -> Instant {
        self.creation_time
    }

    /// Retrieves a config variable.
    ///
    /// This supports most serde `Deserialize` types. Examples:
    ///
    /// ```rust,ignore
    /// let v: Option<u32> = config.get("some.nested.key")?;
    /// let v: Option<MyStruct> = config.get("some.key")?;
    /// let v: Option<HashMap<String, MyStruct>> = config.get("foo")?;
    /// ```
    ///
    /// The key may be a dotted key, but this does NOT support TOML key
    /// quoting. Avoid key components that may have dots. For example,
    /// `foo.'a.b'.bar" does not work if you try to fetch `foo.'a.b'". You can
    /// fetch `foo` if it is a map, though.
    pub fn get<'de, T: serde::de::Deserialize<'de>>(&self, key: &str) -> CargoResult<T> {
        let d = Deserializer {
            gctx: self,
            key: ConfigKey::from_str(key),
            env_prefix_ok: true,
        };
        T::deserialize(d).map_err(|e| e.into())
    }

    /// Obtain a [`Path`] from a [`Filesystem`], verifying that the
    /// appropriate lock is already currently held.
    ///
    /// Locks are usually acquired via [`GlobalContext::acquire_package_cache_lock`]
    /// or [`GlobalContext::try_acquire_package_cache_lock`].
    #[track_caller]
    #[tracing::instrument(skip_all)]
    pub fn assert_package_cache_locked<'a>(
        &self,
        mode: CacheLockMode,
        f: &'a Filesystem,
    ) -> &'a Path {
        let ret = f.as_path_unlocked();
        assert!(
            self.package_cache_lock.is_locked(mode),
            "package cache lock is not currently held, Cargo forgot to call \
             `acquire_package_cache_lock` before we got to this stack frame",
        );
        assert!(ret.starts_with(self.home_path.as_path_unlocked()));
        ret
    }

    /// Acquires a lock on the global "package cache", blocking if another
    /// cargo holds the lock.
    ///
    /// See [`crate::util::cache_lock`] for an in-depth discussion of locking
    /// and lock modes.
    #[tracing::instrument(skip_all)]
    pub fn acquire_package_cache_lock(&self, mode: CacheLockMode) -> CargoResult<CacheLock<'_>> {
        self.package_cache_lock.lock(self, mode)
    }

    /// Acquires a lock on the global "package cache", returning `None` if
    /// another cargo holds the lock.
    ///
    /// See [`crate::util::cache_lock`] for an in-depth discussion of locking
    /// and lock modes.
    #[tracing::instrument(skip_all)]
    pub fn try_acquire_package_cache_lock(
        &self,
        mode: CacheLockMode,
    ) -> CargoResult<Option<CacheLock<'_>>> {
        self.package_cache_lock.try_lock(self, mode)
    }

    /// Returns a reference to the shared [`GlobalCacheTracker`].
    ///
    /// The package cache lock must be held to call this function (and to use
    /// it in general).
    pub fn global_cache_tracker(&self) -> CargoResult<MutexGuard<'_, GlobalCacheTracker>> {
        let tracker = self.global_cache_tracker.try_borrow_with(|| {
            Ok::<_, anyhow::Error>(Mutex::new(GlobalCacheTracker::new(self)?))
        })?;
        Ok(tracker.lock().unwrap())
    }

    /// Returns a reference to the shared [`DeferredGlobalLastUse`].
    pub fn deferred_global_last_use(&self) -> CargoResult<MutexGuard<'_, DeferredGlobalLastUse>> {
        let deferred = self
            .deferred_global_last_use
            .try_borrow_with(|| Ok::<_, anyhow::Error>(Mutex::new(DeferredGlobalLastUse::new())))?;
        Ok(deferred.lock().unwrap())
    }

    /// Get the global [`WarningHandling`] configuration.
    pub fn warning_handling(&self) -> CargoResult<WarningHandling> {
        if self.unstable_flags.warnings {
            Ok(self.build_config()?.warnings.unwrap_or_default())
        } else {
            Ok(WarningHandling::default())
        }
    }

    pub fn ws_roots(&self) -> MutexGuard<'_, HashMap<PathBuf, WorkspaceRootConfig>> {
        self.ws_roots.lock().unwrap()
    }
}

pub fn homedir(cwd: &Path) -> Option<PathBuf> {
    ::home::cargo_home_with_cwd(cwd).ok()
}

pub fn save_credentials(
    gctx: &GlobalContext,
    token: Option<RegistryCredentialConfig>,
    registry: &SourceId,
) -> CargoResult<()> {
    let registry = if registry.is_crates_io() {
        None
    } else {
        let name = registry
            .alt_registry_key()
            .ok_or_else(|| internal("can't save credentials for anonymous registry"))?;
        Some(name)
    };

    // If 'credentials' exists, write to that for backward compatibility reasons.
    // Otherwise write to 'credentials.toml'. There's no need to print the
    // warning here, because it would already be printed at load time.
    let home_path = gctx.home_path.clone().into_path_unlocked();
    let filename = match gctx.get_file_path(&home_path, "credentials", false)? {
        Some(path) => match path.file_name() {
            Some(filename) => Path::new(filename).to_owned(),
            None => Path::new("credentials.toml").to_owned(),
        },
        None => Path::new("credentials.toml").to_owned(),
    };

    let mut file = {
        gctx.home_path.create_dir()?;
        gctx.home_path
            .open_rw_exclusive_create(filename, gctx, "credentials' config file")?
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents).with_context(|| {
        format!(
            "failed to read configuration file `{}`",
            file.path().display()
        )
    })?;

    let mut toml = parse_document(&contents, file.path(), gctx)?;

    // Move the old token location to the new one.
    if let Some(token) = toml.remove("token") {
        let map = HashMap::from([("token".to_string(), token)]);
        toml.insert("registry".into(), map.into());
    }

    if let Some(token) = token {
        // login

        let path_def = Definition::Path(file.path().to_path_buf());
        let (key, mut value) = match token {
            RegistryCredentialConfig::Token(token) => {
                // login with token

                let key = "token".to_string();
                let value = ConfigValue::String(token.expose(), path_def.clone());
                let map = HashMap::from([(key, value)]);
                let table = CV::Table(map, path_def.clone());

                if let Some(registry) = registry {
                    let map = HashMap::from([(registry.to_string(), table)]);
                    ("registries".into(), CV::Table(map, path_def.clone()))
                } else {
                    ("registry".into(), table)
                }
            }
            RegistryCredentialConfig::AsymmetricKey((secret_key, key_subject)) => {
                // login with key

                let key = "secret-key".to_string();
                let value = ConfigValue::String(secret_key.expose(), path_def.clone());
                let mut map = HashMap::from([(key, value)]);
                if let Some(key_subject) = key_subject {
                    let key = "secret-key-subject".to_string();
                    let value = ConfigValue::String(key_subject, path_def.clone());
                    map.insert(key, value);
                }
                let table = CV::Table(map, path_def.clone());

                if let Some(registry) = registry {
                    let map = HashMap::from([(registry.to_string(), table)]);
                    ("registries".into(), CV::Table(map, path_def.clone()))
                } else {
                    ("registry".into(), table)
                }
            }
            _ => unreachable!(),
        };

        if registry.is_some() {
            if let Some(table) = toml.remove("registries") {
                let v = CV::from_toml(path_def, table)?;
                value.merge(v, false)?;
            }
        }
        toml.insert(key, value.into_toml());
    } else {
        // logout
        if let Some(registry) = registry {
            if let Some(registries) = toml.get_mut("registries") {
                if let Some(reg) = registries.get_mut(registry) {
                    let rtable = reg.as_table_mut().ok_or_else(|| {
                        format_err!("expected `[registries.{}]` to be a table", registry)
                    })?;
                    rtable.remove("token");
                    rtable.remove("secret-key");
                    rtable.remove("secret-key-subject");
                }
            }
        } else if let Some(registry) = toml.get_mut("registry") {
            let reg_table = registry
                .as_table_mut()
                .ok_or_else(|| format_err!("expected `[registry]` to be a table"))?;
            reg_table.remove("token");
            reg_table.remove("secret-key");
            reg_table.remove("secret-key-subject");
        }
    }

    let contents = toml.to_string();
    file.seek(SeekFrom::Start(0))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write to `{}`", file.path().display()))?;
    file.file().set_len(contents.len() as u64)?;
    set_permissions(file.file(), 0o600)
        .with_context(|| format!("failed to set permissions of `{}`", file.path().display()))?;

    return Ok(());

    #[cfg(unix)]
    fn set_permissions(file: &File, mode: u32) -> CargoResult<()> {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = file.metadata()?.permissions();
        perms.set_mode(mode);
        file.set_permissions(perms)?;
        Ok(())
    }

    #[cfg(not(unix))]
    #[allow(unused)]
    fn set_permissions(file: &File, mode: u32) -> CargoResult<()> {
        Ok(())
    }
}

/// Represents a config-include value in the configuration.
///
/// This intentionally doesn't derive serde deserialization
/// to avoid any misuse of `GlobalContext::get::<ConfigInclude>()`,
/// which might lead to wrong config loading order.
struct ConfigInclude {
    /// Path to a config-include configuration file.
    /// Could be either relative or absolute.
    path: PathBuf,
    def: Definition,
    /// Whether this include is optional (missing files are silently ignored)
    optional: bool,
}

impl ConfigInclude {
    fn new(p: impl Into<PathBuf>, def: Definition) -> Self {
        Self {
            path: p.into(),
            def,
            optional: false,
        }
    }

    /// Resolves the absolute path for this include.
    ///
    /// For file based include,
    /// it is relative to parent directory of the config file includes it.
    /// For example, if `.cargo/config.toml has a `include = "foo.toml"`,
    /// Cargo will load `.cargo/foo.toml`.
    ///
    /// For CLI based include (e.g., `--config 'include = "foo.toml"'`),
    /// it is relative to the current working directory.
    ///
    /// Returns `None` if this is an optional include and the file doesn't exist.
    /// Otherwise returns `Some(PathBuf)` with the absolute path.
    fn resolve_path(&self, gctx: &GlobalContext) -> Option<PathBuf> {
        let abs_path = match &self.def {
            Definition::Path(p) | Definition::Cli(Some(p)) => p.parent().unwrap(),
            Definition::Environment(_) | Definition::Cli(None) | Definition::BuiltIn => gctx.cwd(),
        }
        .join(&self.path);

        if self.optional && !abs_path.exists() {
            tracing::info!(
                "skipping optional include `{}` in `{}`:  file not found at `{}`",
                self.path.display(),
                self.def,
                abs_path.display(),
            );
            None
        } else {
            Some(abs_path)
        }
    }
}

fn parse_document(toml: &str, _file: &Path, _gctx: &GlobalContext) -> CargoResult<toml::Table> {
    // At the moment, no compatibility checks are needed.
    toml.parse().map_err(Into::into)
}

fn toml_dotted_keys(arg: &str) -> CargoResult<toml_edit::DocumentMut> {
    // We only want to allow "dotted key" (see https://toml.io/en/v1.0.0#keys)
    // expressions followed by a value that's not an "inline table"
    // (https://toml.io/en/v1.0.0#inline-table). Easiest way to check for that is to
    // parse the value as a toml_edit::DocumentMut, and check that the (single)
    // inner-most table is set via dotted keys.
    let doc: toml_edit::DocumentMut = arg.parse().with_context(|| {
        format!("failed to parse value from --config argument `{arg}` as a dotted key expression")
    })?;
    fn non_empty(d: Option<&toml_edit::RawString>) -> bool {
        d.map_or(false, |p| !p.as_str().unwrap_or_default().trim().is_empty())
    }
    fn non_empty_decor(d: &toml_edit::Decor) -> bool {
        non_empty(d.prefix()) || non_empty(d.suffix())
    }
    fn non_empty_key_decor(k: &toml_edit::Key) -> bool {
        non_empty_decor(k.leaf_decor()) || non_empty_decor(k.dotted_decor())
    }
    let ok = {
        let mut got_to_value = false;
        let mut table = doc.as_table();
        let mut is_root = true;
        while table.is_dotted() || is_root {
            is_root = false;
            if table.len() != 1 {
                break;
            }
            let (k, n) = table.iter().next().expect("len() == 1 above");
            match n {
                Item::Table(nt) => {
                    if table.key(k).map_or(false, non_empty_key_decor)
                        || non_empty_decor(nt.decor())
                    {
                        bail!(
                            "--config argument `{arg}` \
                                includes non-whitespace decoration"
                        )
                    }
                    table = nt;
                }
                Item::Value(v) if v.is_inline_table() => {
                    bail!(
                        "--config argument `{arg}` \
                        sets a value to an inline table, which is not accepted"
                    );
                }
                Item::Value(v) => {
                    if table
                        .key(k)
                        .map_or(false, |k| non_empty(k.leaf_decor().prefix()))
                        || non_empty_decor(v.decor())
                    {
                        bail!(
                            "--config argument `{arg}` \
                                includes non-whitespace decoration"
                        )
                    }
                    got_to_value = true;
                    break;
                }
                Item::ArrayOfTables(_) => {
                    bail!(
                        "--config argument `{arg}` \
                        sets a value to an array of tables, which is not accepted"
                    );
                }

                Item::None => {
                    bail!("--config argument `{arg}` doesn't provide a value")
                }
            }
        }
        got_to_value
    };
    if !ok {
        bail!(
            "--config argument `{arg}` was not a TOML dotted key expression (such as `build.jobs = 2`)"
        );
    }
    Ok(doc)
}

/// A type to deserialize a list of strings from a toml file.
///
/// Supports deserializing either a whitespace-separated list of arguments in a
/// single string or a string list itself. For example these deserialize to
/// equivalent values:
///
/// ```toml
/// a = 'a b c'
/// b = ['a', 'b', 'c']
/// ```
#[derive(Debug, Deserialize, Clone)]
pub struct StringList(Vec<String>);

impl StringList {
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

#[macro_export]
macro_rules! __shell_print {
    ($config:expr, $which:ident, $newline:literal, $($arg:tt)*) => ({
        let mut shell = $config.shell();
        let out = shell.$which();
        drop(out.write_fmt(format_args!($($arg)*)));
        if $newline {
            drop(out.write_all(b"\n"));
        }
    });
}

#[macro_export]
macro_rules! drop_println {
    ($config:expr) => ( $crate::drop_print!($config, "\n") );
    ($config:expr, $($arg:tt)*) => (
        $crate::__shell_print!($config, out, true, $($arg)*)
    );
}

#[macro_export]
macro_rules! drop_eprintln {
    ($config:expr) => ( $crate::drop_eprint!($config, "\n") );
    ($config:expr, $($arg:tt)*) => (
        $crate::__shell_print!($config, err, true, $($arg)*)
    );
}

#[macro_export]
macro_rules! drop_print {
    ($config:expr, $($arg:tt)*) => (
        $crate::__shell_print!($config, out, false, $($arg)*)
    );
}

#[macro_export]
macro_rules! drop_eprint {
    ($config:expr, $($arg:tt)*) => (
        $crate::__shell_print!($config, err, false, $($arg)*)
    );
}

enum Tool {
    Rustc,
    Rustdoc,
}

impl Tool {
    fn as_str(&self) -> &str {
        match self {
            Tool::Rustc => "rustc",
            Tool::Rustdoc => "rustdoc",
        }
    }
}

/// Disable HTTP/2 multiplexing for some broken versions of libcurl.
///
/// In certain versions of libcurl when proxy is in use with HTTP/2
/// multiplexing, connections will continue stacking up. This was
/// fixed in libcurl 8.0.0 in curl/curl@821f6e2a89de8aec1c7da3c0f381b92b2b801efc
///
/// However, Cargo can still link against old system libcurl if it is from a
/// custom built one or on macOS. For those cases, multiplexing needs to be
/// disabled when those versions are detected.
fn disables_multiplexing_for_bad_curl(
    curl_version: &str,
    http: &mut CargoHttpConfig,
    gctx: &GlobalContext,
) {
    use crate::util::network;

    if network::proxy::http_proxy_exists(http, gctx) && http.multiplexing.is_none() {
        let bad_curl_versions = ["7.87.0", "7.88.0", "7.88.1"];
        if bad_curl_versions
            .iter()
            .any(|v| curl_version.starts_with(v))
        {
            tracing::info!("disabling multiplexing with proxy, curl version is {curl_version}");
            http.multiplexing = Some(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CargoHttpConfig;
    use super::GlobalContext;
    use super::Shell;
    use super::disables_multiplexing_for_bad_curl;

    #[test]
    fn disables_multiplexing() {
        let mut gctx = GlobalContext::new(Shell::new(), "".into(), "".into());
        gctx.set_search_stop_path(std::path::PathBuf::new());
        gctx.set_env(Default::default());

        let mut http = CargoHttpConfig::default();
        http.proxy = Some("127.0.0.1:3128".into());
        disables_multiplexing_for_bad_curl("7.88.1", &mut http, &gctx);
        assert_eq!(http.multiplexing, Some(false));

        let cases = [
            (None, None, "7.87.0", None),
            (None, None, "7.88.0", None),
            (None, None, "7.88.1", None),
            (None, None, "8.0.0", None),
            (Some("".into()), None, "7.87.0", Some(false)),
            (Some("".into()), None, "7.88.0", Some(false)),
            (Some("".into()), None, "7.88.1", Some(false)),
            (Some("".into()), None, "8.0.0", None),
            (Some("".into()), Some(false), "7.87.0", Some(false)),
            (Some("".into()), Some(false), "7.88.0", Some(false)),
            (Some("".into()), Some(false), "7.88.1", Some(false)),
            (Some("".into()), Some(false), "8.0.0", Some(false)),
        ];

        for (proxy, multiplexing, curl_v, result) in cases {
            let mut http = CargoHttpConfig {
                multiplexing,
                proxy,
                ..Default::default()
            };
            disables_multiplexing_for_bad_curl(curl_v, &mut http, &gctx);
            assert_eq!(http.multiplexing, result);
        }
    }

    #[test]
    fn sync_context() {
        fn assert_sync<S: Sync>() {}
        assert_sync::<GlobalContext>();
    }
}
