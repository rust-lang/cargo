use std::cell::{RefCell, RefMut};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_map::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::mem;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Once, ONCE_INIT};
use std::time::Instant;
use std::vec;

use curl::easy::Easy;
use lazycell::LazyCell;
use serde::Deserialize;
use serde::{de, de::IntoDeserializer};
use url::Url;

use crate::core::profiles::ConfigProfiles;
use crate::core::shell::Verbosity;
use crate::core::{CliUnstable, Shell, SourceId, Workspace};
use crate::ops;
use crate::util::errors::{internal, CargoResult, CargoResultExt};
use crate::util::toml as cargo_toml;
use crate::util::Filesystem;
use crate::util::Rustc;
use crate::util::ToUrl;
use crate::util::{paths, validate_package_name};
use self::ConfigValue as CV;

/// Configuration information for cargo. This is not specific to a build, it is information
/// relating to cargo itself.
///
/// This struct implements `Default`: all fields can be inferred.
#[derive(Debug)]
pub struct Config {
    /// The location of the user's 'home' directory. OS-dependent.
    home_path: Filesystem,
    /// Information about how to write messages to the shell
    shell: RefCell<Shell>,
    /// A collection of configuration options
    values: LazyCell<HashMap<String, ConfigValue>>,
    /// The current working directory of cargo
    cwd: PathBuf,
    /// The location of the cargo executable (path to current process)
    cargo_exe: LazyCell<PathBuf>,
    /// The location of the rustdoc executable
    rustdoc: LazyCell<PathBuf>,
    /// Whether we are printing extra verbose messages
    extra_verbose: bool,
    /// `frozen` is set if we shouldn't access the network
    frozen: bool,
    /// `locked` is set if we should not update lock files
    locked: bool,
    /// A global static IPC control mechanism (used for managing parallel builds)
    jobserver: Option<jobserver::Client>,
    /// Cli flags of the form "-Z something"
    cli_flags: CliUnstable,
    /// A handle on curl easy mode for http calls
    easy: LazyCell<RefCell<Easy>>,
    /// Cache of the `SourceId` for crates.io
    crates_io_source_id: LazyCell<SourceId>,
    /// If false, don't cache `rustc --version --verbose` invocations
    cache_rustc_info: bool,
    /// Creation time of this config, used to output the total build time
    creation_time: Instant,
    /// Target Directory via resolved Cli parameter
    target_dir: Option<Filesystem>,
    /// Environment variables, separated to assist testing.
    env: HashMap<String, String>,
    /// Profiles loaded from config.
    profiles: LazyCell<ConfigProfiles>,
}

impl Config {
    pub fn new(shell: Shell, cwd: PathBuf, homedir: PathBuf) -> Config {
        static mut GLOBAL_JOBSERVER: *mut jobserver::Client = 0 as *mut _;
        static INIT: Once = ONCE_INIT;

        // This should be called early on in the process, so in theory the
        // unsafety is ok here. (taken ownership of random fds)
        INIT.call_once(|| unsafe {
            if let Some(client) = jobserver::Client::from_env() {
                GLOBAL_JOBSERVER = Box::into_raw(Box::new(client));
            }
        });

        let env: HashMap<_, _> = env::vars_os()
            .filter_map(|(k, v)| {
                // Ignore any key/values that are not valid Unicode.
                match (k.into_string(), v.into_string()) {
                    (Ok(k), Ok(v)) => Some((k, v)),
                    _ => None,
                }
            })
            .collect();

        let cache_rustc_info = match env.get("CARGO_CACHE_RUSTC_INFO") {
            Some(cache) => cache != "0",
            _ => true,
        };

        Config {
            home_path: Filesystem::new(homedir),
            shell: RefCell::new(shell),
            cwd,
            values: LazyCell::new(),
            cargo_exe: LazyCell::new(),
            rustdoc: LazyCell::new(),
            extra_verbose: false,
            frozen: false,
            locked: false,
            jobserver: unsafe {
                if GLOBAL_JOBSERVER.is_null() {
                    None
                } else {
                    Some((*GLOBAL_JOBSERVER).clone())
                }
            },
            cli_flags: CliUnstable::default(),
            easy: LazyCell::new(),
            crates_io_source_id: LazyCell::new(),
            cache_rustc_info,
            creation_time: Instant::now(),
            target_dir: None,
            env,
            profiles: LazyCell::new(),
        }
    }

    pub fn default() -> CargoResult<Config> {
        let shell = Shell::new();
        let cwd =
            env::current_dir().chain_err(|| "couldn't get the current directory of the process")?;
        let homedir = homedir(&cwd).ok_or_else(|| {
            failure::format_err!(
                "Cargo couldn't find your home directory. \
                 This probably means that $HOME was not set."
            )
        })?;
        Ok(Config::new(shell, cwd, homedir))
    }

    /// Gets the user's Cargo home directory (OS-dependent).
    pub fn home(&self) -> &Filesystem {
        &self.home_path
    }

    /// Gets the Cargo Git directory (`<cargo_home>/git`).
    pub fn git_path(&self) -> Filesystem {
        self.home_path.join("git")
    }

    /// Gets the Cargo registry index directory (`<cargo_home>/registry/index`).
    pub fn registry_index_path(&self) -> Filesystem {
        self.home_path.join("registry").join("index")
    }

    /// Gets the Cargo registry cache directory (`<cargo_home>/registry/path`).
    pub fn registry_cache_path(&self) -> Filesystem {
        self.home_path.join("registry").join("cache")
    }

    /// Gets the Cargo registry source directory (`<cargo_home>/registry/src`).
    pub fn registry_source_path(&self) -> Filesystem {
        self.home_path.join("registry").join("src")
    }

    /// Gets the default Cargo registry.
    pub fn default_registry(&self) -> CargoResult<Option<String>> {
        Ok(match self.get_string("registry.default")? {
            Some(registry) => Some(registry.val),
            None => None,
        })
    }

    /// Gets a reference to the shell, e.g., for writing error messages.
    pub fn shell(&self) -> RefMut<'_, Shell> {
        self.shell.borrow_mut()
    }

    /// Gets the path to the `rustdoc` executable.
    pub fn rustdoc(&self) -> CargoResult<&Path> {
        self.rustdoc
            .try_borrow_with(|| self.get_tool("rustdoc"))
            .map(AsRef::as_ref)
    }

    /// Gets the path to the `rustc` executable.
    pub fn rustc(&self, ws: Option<&Workspace<'_>>) -> CargoResult<Rustc> {
        let cache_location = ws.map(|ws| {
            ws.target_dir()
                .join(".rustc_info.json")
                .into_path_unlocked()
        });
        Rustc::new(
            self.get_tool("rustc")?,
            self.maybe_get_tool("rustc_wrapper")?,
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
        )
    }

    /// Gets the path to the `cargo` executable.
    pub fn cargo_exe(&self) -> CargoResult<&Path> {
        self.cargo_exe
            .try_borrow_with(|| {
                fn from_current_exe() -> CargoResult<PathBuf> {
                    // Try fetching the path to `cargo` using `env::current_exe()`.
                    // The method varies per operating system and might fail; in particular,
                    // it depends on `/proc` being mounted on Linux, and some environments
                    // (like containers or chroots) may not have that available.
                    let exe = env::current_exe()?.canonicalize()?;
                    Ok(exe)
                }

                fn from_argv() -> CargoResult<PathBuf> {
                    // Grab `argv[0]` and attempt to resolve it to an absolute path.
                    // If `argv[0]` has one component, it must have come from a `PATH` lookup,
                    // so probe `PATH` in that case.
                    // Otherwise, it has multiple components and is either:
                    // - a relative path (e.g., `./cargo`, `target/debug/cargo`), or
                    // - an absolute path (e.g., `/usr/local/bin/cargo`).
                    // In either case, `Path::canonicalize` will return the full absolute path
                    // to the target if it exists.
                    let argv0 = env::args_os()
                        .map(PathBuf::from)
                        .next()
                        .ok_or_else(|| failure::format_err!("no argv[0]"))?;
                    paths::resolve_executable(&argv0)
                }

                let exe = from_current_exe()
                    .or_else(|_| from_argv())
                    .chain_err(|| "couldn't get the path to cargo executable")?;
                Ok(exe)
            })
            .map(AsRef::as_ref)
    }

    pub fn profiles(&self) -> CargoResult<&ConfigProfiles> {
        self.profiles.try_borrow_with(|| {
            let ocp = self.get::<Option<ConfigProfiles>>("profile")?;
            if let Some(config_profiles) = ocp {
                // Warn if config profiles without CLI option.
                if !self.cli_unstable().config_profile {
                    self.shell().warn(
                        "profiles in config files require `-Z config-profile` \
                         command-line option",
                    )?;
                    return Ok(ConfigProfiles::default());
                }
                Ok(config_profiles)
            } else {
                Ok(ConfigProfiles::default())
            }
        })
    }

    pub fn values(&self) -> CargoResult<&HashMap<String, ConfigValue>> {
        self.values.try_borrow_with(|| self.load_values())
    }

    // Note: this is used by RLS, not Cargo.
    pub fn set_values(&self, values: HashMap<String, ConfigValue>) -> CargoResult<()> {
        if self.values.borrow().is_some() {
            failure::bail!("config values already found")
        }
        match self.values.fill(values) {
            Ok(()) => Ok(()),
            Err(_) => failure::bail!("could not fill values"),
        }
    }

    pub fn reload_rooted_at_cargo_home(&mut self) -> CargoResult<()> {
        let home = self.home_path.clone().into_path_unlocked();
        let values = self.load_values_from(&home)?;
        self.values.replace(values);
        Ok(())
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn target_dir(&self) -> CargoResult<Option<Filesystem>> {
        if let Some(ref dir) = self.target_dir {
            Ok(Some(dir.clone()))
        } else if let Some(dir) = env::var_os("CARGO_TARGET_DIR") {
            Ok(Some(Filesystem::new(self.cwd.join(dir))))
        } else if let Some(val) = self.get_path("build.target-dir")? {
            let val = self.cwd.join(val.val);
            Ok(Some(Filesystem::new(val)))
        } else {
            Ok(None)
        }
    }

    fn get_cv(&self, key: &str) -> CargoResult<Option<ConfigValue>> {
        let vals = self.values()?;
        let mut parts = key.split('.').enumerate();
        let mut val = match vals.get(parts.next().unwrap().1) {
            Some(val) => val,
            None => return Ok(None),
        };
        for (i, part) in parts {
            match *val {
                CV::Table(ref map, _) => {
                    val = match map.get(part) {
                        Some(val) => val,
                        None => return Ok(None),
                    }
                }
                CV::Integer(_, ref path)
                | CV::String(_, ref path)
                | CV::List(_, ref path)
                | CV::Boolean(_, ref path) => {
                    let idx = key.split('.').take(i).fold(0, |n, s| n + s.len()) + i - 1;
                    let key_so_far = &key[..idx];
                    failure::bail!(
                        "expected table for configuration key `{}`, \
                         but found {} in {}",
                        key_so_far,
                        val.desc(),
                        path.display()
                    )
                }
            }
        }
        Ok(Some(val.clone()))
    }

    // Helper primarily for testing.
    pub fn set_env(&mut self, env: HashMap<String, String>) {
        self.env = env;
    }

    fn get_env<T>(&self, key: &ConfigKey) -> Result<OptValue<T>, ConfigError>
    where
        T: FromStr,
        <T as FromStr>::Err: fmt::Display,
    {
        let key = key.to_env();
        match self.env.get(&key) {
            Some(value) => {
                let definition = Definition::Environment(key);
                Ok(Some(Value {
                    val: value
                        .parse()
                        .map_err(|e| ConfigError::new(format!("{}", e), definition.clone()))?,
                    definition,
                }))
            }
            None => Ok(None),
        }
    }

    fn has_key(&self, key: &ConfigKey) -> bool {
        let env_key = key.to_env();
        if self.env.get(&env_key).is_some() {
            return true;
        }
        let env_pattern = format!("{}_", env_key);
        if self.env.keys().any(|k| k.starts_with(&env_pattern)) {
            return true;
        }
        if let Ok(o_cv) = self.get_cv(&key.to_config()) {
            if o_cv.is_some() {
                return true;
            }
        }
        false
    }

    pub fn get_string(&self, key: &str) -> CargoResult<OptValue<String>> {
        self.get_string_priv(&ConfigKey::from_str(key))
            .map_err(|e| e.into())
    }

    fn get_string_priv(&self, key: &ConfigKey) -> Result<OptValue<String>, ConfigError> {
        match self.get_env(key)? {
            Some(v) => Ok(Some(v)),
            None => {
                let config_key = key.to_config();
                let o_cv = self.get_cv(&config_key)?;
                match o_cv {
                    Some(CV::String(s, path)) => Ok(Some(Value {
                        val: s,
                        definition: Definition::Path(path),
                    })),
                    Some(cv) => Err(ConfigError::expected(&config_key, "a string", &cv)),
                    None => Ok(None),
                }
            }
        }
    }

    pub fn get_bool(&self, key: &str) -> CargoResult<OptValue<bool>> {
        self.get_bool_priv(&ConfigKey::from_str(key))
            .map_err(|e| e.into())
    }

    fn get_bool_priv(&self, key: &ConfigKey) -> Result<OptValue<bool>, ConfigError> {
        match self.get_env(key)? {
            Some(v) => Ok(Some(v)),
            None => {
                let config_key = key.to_config();
                let o_cv = self.get_cv(&config_key)?;
                match o_cv {
                    Some(CV::Boolean(b, path)) => Ok(Some(Value {
                        val: b,
                        definition: Definition::Path(path),
                    })),
                    Some(cv) => Err(ConfigError::expected(&config_key, "true/false", &cv)),
                    None => Ok(None),
                }
            }
        }
    }

    fn string_to_path(&self, value: String, definition: &Definition) -> PathBuf {
        let is_path = value.contains('/') || (cfg!(windows) && value.contains('\\'));
        if is_path {
            definition.root(self).join(value)
        } else {
            // A pathless name.
            PathBuf::from(value)
        }
    }

    pub fn get_path(&self, key: &str) -> CargoResult<OptValue<PathBuf>> {
        if let Some(val) = self.get_string(key)? {
            Ok(Some(Value {
                val: self.string_to_path(val.val, &val.definition),
                definition: val.definition,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_path_and_args(&self, key: &str) -> CargoResult<OptValue<(PathBuf, Vec<String>)>> {
        if let Some(mut val) = self.get_list_or_split_string(key)? {
            if !val.val.is_empty() {
                return Ok(Some(Value {
                    val: (
                        self.string_to_path(val.val.remove(0), &val.definition),
                        val.val,
                    ),
                    definition: val.definition,
                }));
            }
        }
        Ok(None)
    }

    // NOTE: this does **not** support environment variables. Use `get` instead
    // if you want that.
    pub fn get_list(&self, key: &str) -> CargoResult<OptValue<Vec<(String, PathBuf)>>> {
        match self.get_cv(key)? {
            Some(CV::List(i, path)) => Ok(Some(Value {
                val: i,
                definition: Definition::Path(path),
            })),
            Some(val) => self.expected("list", key, &val),
            None => Ok(None),
        }
    }

    pub fn get_list_or_split_string(&self, key: &str) -> CargoResult<OptValue<Vec<String>>> {
        if let Some(value) = self.get_env::<String>(&ConfigKey::from_str(key))? {
            return Ok(Some(Value {
                val: value.val.split(' ').map(str::to_string).collect(),
                definition: value.definition,
            }));
        }

        match self.get_cv(key)? {
            Some(CV::List(i, path)) => Ok(Some(Value {
                val: i.into_iter().map(|(s, _)| s).collect(),
                definition: Definition::Path(path),
            })),
            Some(CV::String(i, path)) => Ok(Some(Value {
                val: i.split(' ').map(str::to_string).collect(),
                definition: Definition::Path(path),
            })),
            Some(val) => self.expected("list or string", key, &val),
            None => Ok(None),
        }
    }

    pub fn get_table(&self, key: &str) -> CargoResult<OptValue<HashMap<String, CV>>> {
        match self.get_cv(key)? {
            Some(CV::Table(i, path)) => Ok(Some(Value {
                val: i,
                definition: Definition::Path(path),
            })),
            Some(val) => self.expected("table", key, &val),
            None => Ok(None),
        }
    }

    // Recommended to use `get` if you want a specific type, such as an unsigned value.
    // Example: `config.get::<Option<u32>>("some.key")?`.
    pub fn get_i64(&self, key: &str) -> CargoResult<OptValue<i64>> {
        self.get_integer(&ConfigKey::from_str(key))
            .map_err(|e| e.into())
    }

    fn get_integer(&self, key: &ConfigKey) -> Result<OptValue<i64>, ConfigError> {
        let config_key = key.to_config();
        match self.get_env::<i64>(key)? {
            Some(v) => Ok(Some(v)),
            None => match self.get_cv(&config_key)? {
                Some(CV::Integer(i, path)) => Ok(Some(Value {
                    val: i,
                    definition: Definition::Path(path),
                })),
                Some(cv) => Err(ConfigError::expected(&config_key, "an integer", &cv)),
                None => Ok(None),
            },
        }
    }

    fn expected<T>(&self, ty: &str, key: &str, val: &CV) -> CargoResult<T> {
        val.expected(ty, key)
            .map_err(|e| failure::format_err!("invalid configuration for key `{}`\n{}", key, e))
    }

    pub fn configure(
        &mut self,
        verbose: u32,
        quiet: Option<bool>,
        color: &Option<String>,
        frozen: bool,
        locked: bool,
        target_dir: &Option<PathBuf>,
        unstable_flags: &[String],
    ) -> CargoResult<()> {
        let extra_verbose = verbose >= 2;
        let verbose = if verbose == 0 { None } else { Some(true) };

        // Ignore errors in the configuration files.
        let cfg_verbose = self.get_bool("term.verbose").unwrap_or(None).map(|v| v.val);
        let cfg_color = self.get_string("term.color").unwrap_or(None).map(|v| v.val);

        let color = color.as_ref().or_else(|| cfg_color.as_ref());

        let verbosity = match (verbose, cfg_verbose, quiet) {
            (Some(true), _, None) | (None, Some(true), None) => Verbosity::Verbose,

            // Command line takes precedence over configuration, so ignore the
            // configuration..
            (None, _, Some(true)) => Verbosity::Quiet,

            // Can't pass both at the same time on the command line regardless
            // of configuration.
            (Some(true), _, Some(true)) => {
                failure::bail!("cannot set both --verbose and --quiet");
            }

            // Can't actually get `Some(false)` as a value from the command
            // line, so just ignore them here to appease exhaustiveness checking
            // in match statements.
            (Some(false), _, _)
            | (_, _, Some(false))
            | (None, Some(false), None)
            | (None, None, None) => Verbosity::Normal,
        };

        let cli_target_dir = match target_dir.as_ref() {
            Some(dir) => Some(Filesystem::new(dir.clone())),
            None => None,
        };

        self.shell().set_verbosity(verbosity);
        self.shell().set_color_choice(color.map(|s| &s[..]))?;
        self.extra_verbose = extra_verbose;
        self.frozen = frozen;
        self.locked = locked;
        self.target_dir = cli_target_dir;
        self.cli_flags.parse(unstable_flags)?;

        Ok(())
    }

    pub fn cli_unstable(&self) -> &CliUnstable {
        &self.cli_flags
    }

    pub fn extra_verbose(&self) -> bool {
        self.extra_verbose
    }

    pub fn network_allowed(&self) -> bool {
        !self.frozen() && !self.cli_unstable().offline
    }

    pub fn frozen(&self) -> bool {
        self.frozen
    }

    pub fn lock_update_allowed(&self) -> bool {
        !self.frozen && !self.locked
    }

    /// Loads configuration from the filesystem.
    pub fn load_values(&self) -> CargoResult<HashMap<String, ConfigValue>> {
        self.load_values_from(&self.cwd)
    }

    fn load_values_from(&self, path: &Path) -> CargoResult<HashMap<String, ConfigValue>> {
        let mut cfg = CV::Table(HashMap::new(), PathBuf::from("."));
        let home = self.home_path.clone().into_path_unlocked();

        walk_tree(path, &home, |path| {
            let mut contents = String::new();
            let mut file = File::open(&path)?;
            file.read_to_string(&mut contents)
                .chain_err(|| format!("failed to read configuration file `{}`", path.display()))?;
            let toml = cargo_toml::parse(&contents, path, self).chain_err(|| {
                format!("could not parse TOML configuration in `{}`", path.display())
            })?;
            let value = CV::from_toml(path, toml).chain_err(|| {
                format!(
                    "failed to load TOML configuration from `{}`",
                    path.display()
                )
            })?;
            cfg.merge(value)
                .chain_err(|| format!("failed to merge configuration at `{}`", path.display()))?;
            Ok(())
        })
        .chain_err(|| "could not load Cargo configuration")?;

        self.load_credentials(&mut cfg)?;
        match cfg {
            CV::Table(map, _) => Ok(map),
            _ => unreachable!(),
        }
    }

    /// Gets the index for a registry.
    pub fn get_registry_index(&self, registry: &str) -> CargoResult<Url> {
        validate_package_name(registry, "registry name", "")?;
        Ok(
            match self.get_string(&format!("registries.{}.index", registry))? {
                Some(index) => {
                    let url = index.val.to_url()?;
                    if url.password().is_some() {
                        failure::bail!("Registry URLs may not contain passwords");
                    }
                    url
                }
                None => failure::bail!("No index found for registry: `{}`", registry),
            },
        )
    }

    /// Loads credentials config from the credentials file into the `ConfigValue` object, if
    /// present.
    fn load_credentials(&self, cfg: &mut ConfigValue) -> CargoResult<()> {
        let home_path = self.home_path.clone().into_path_unlocked();
        let credentials = home_path.join("credentials");
        if fs::metadata(&credentials).is_err() {
            return Ok(());
        }

        let mut contents = String::new();
        let mut file = File::open(&credentials)?;
        file.read_to_string(&mut contents).chain_err(|| {
            format!(
                "failed to read configuration file `{}`",
                credentials.display()
            )
        })?;

        let toml = cargo_toml::parse(&contents, &credentials, self).chain_err(|| {
            format!(
                "could not parse TOML configuration in `{}`",
                credentials.display()
            )
        })?;

        let mut value = CV::from_toml(&credentials, toml).chain_err(|| {
            format!(
                "failed to load TOML configuration from `{}`",
                credentials.display()
            )
        })?;

        // Backwards compatibility for old `.cargo/credentials` layout.
        {
            let value = match value {
                CV::Table(ref mut value, _) => value,
                _ => unreachable!(),
            };

            if let Some(token) = value.remove("token") {
                if let Vacant(entry) = value.entry("registry".into()) {
                    let mut map = HashMap::new();
                    map.insert("token".into(), token);
                    let table = CV::Table(map, PathBuf::from("."));
                    entry.insert(table);
                }
            }
        }

        // We want value to override `cfg`, so swap these.
        mem::swap(cfg, &mut value);
        cfg.merge(value)?;

        Ok(())
    }

    /// Looks for a path for `tool` in an environment variable or config path, and returns `None`
    /// if it's not present.
    fn maybe_get_tool(&self, tool: &str) -> CargoResult<Option<PathBuf>> {
        let var = tool
            .chars()
            .flat_map(|c| c.to_uppercase())
            .collect::<String>();
        if let Some(tool_path) = env::var_os(&var) {
            let maybe_relative = match tool_path.to_str() {
                Some(s) => s.contains('/') || s.contains('\\'),
                None => false,
            };
            let path = if maybe_relative {
                self.cwd.join(tool_path)
            } else {
                PathBuf::from(tool_path)
            };
            return Ok(Some(path));
        }

        let var = format!("build.{}", tool);
        if let Some(tool_path) = self.get_path(&var)? {
            return Ok(Some(tool_path.val));
        }

        Ok(None)
    }

    /// Looks for a path for `tool` in an environment variable or config path, defaulting to `tool`
    /// as a path.
    fn get_tool(&self, tool: &str) -> CargoResult<PathBuf> {
        self.maybe_get_tool(tool)
            .map(|t| t.unwrap_or_else(|| PathBuf::from(tool)))
    }

    pub fn jobserver_from_env(&self) -> Option<&jobserver::Client> {
        self.jobserver.as_ref()
    }

    pub fn http(&self) -> CargoResult<&RefCell<Easy>> {
        let http = self
            .easy
            .try_borrow_with(|| ops::http_handle(self).map(RefCell::new))?;
        {
            let mut http = http.borrow_mut();
            http.reset();
            let timeout = ops::configure_http_handle(self, &mut http)?;
            timeout.configure(&mut http)?;
        }
        Ok(http)
    }

    pub fn crates_io_source_id<F>(&self, f: F) -> CargoResult<SourceId>
    where
        F: FnMut() -> CargoResult<SourceId>,
    {
        Ok(*(self.crates_io_source_id.try_borrow_with(f)?))
    }

    pub fn creation_time(&self) -> Instant {
        self.creation_time
    }

    // Retrieves a config variable.
    //
    // This supports most serde `Deserialize` types. Examples:
    //
    //     let v: Option<u32> = config.get("some.nested.key")?;
    //     let v: Option<MyStruct> = config.get("some.key")?;
    //     let v: Option<HashMap<String, MyStruct>> = config.get("foo")?;
    pub fn get<'de, T: de::Deserialize<'de>>(&self, key: &str) -> CargoResult<T> {
        let d = Deserializer {
            config: self,
            key: ConfigKey::from_str(key),
        };
        T::deserialize(d).map_err(|e| e.into())
    }
}

/// A segment of a config key.
///
/// Config keys are split on dots for regular keys, or underscores for
/// environment keys.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum ConfigKeyPart {
    /// Case-insensitive part (checks uppercase in environment keys).
    Part(String),
    /// Case-sensitive part (environment keys must match exactly).
    CasePart(String),
}

impl ConfigKeyPart {
    fn to_env(&self) -> String {
        match self {
            ConfigKeyPart::Part(s) => s.replace("-", "_").to_uppercase(),
            ConfigKeyPart::CasePart(s) => s.clone(),
        }
    }

    fn to_config(&self) -> String {
        match self {
            ConfigKeyPart::Part(s) => s.clone(),
            ConfigKeyPart::CasePart(s) => s.clone(),
        }
    }
}

/// Key for a configuration variable.
#[derive(Debug, Clone)]
struct ConfigKey(Vec<ConfigKeyPart>);

impl ConfigKey {
    fn from_str(key: &str) -> ConfigKey {
        ConfigKey(
            key.split('.')
                .map(|p| ConfigKeyPart::Part(p.to_string()))
                .collect(),
        )
    }

    fn join(&self, next: ConfigKeyPart) -> ConfigKey {
        let mut res = self.clone();
        res.0.push(next);
        res
    }

    fn to_env(&self) -> String {
        format!(
            "CARGO_{}",
            self.0
                .iter()
                .map(|p| p.to_env())
                .collect::<Vec<_>>()
                .join("_")
        )
    }

    fn to_config(&self) -> String {
        self.0
            .iter()
            .map(|p| p.to_config())
            .collect::<Vec<_>>()
            .join(".")
    }

    fn last(&self) -> &ConfigKeyPart {
        self.0.last().unwrap()
    }
}

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_config().fmt(f)
    }
}

/// Internal error for serde errors.
#[derive(Debug)]
pub struct ConfigError {
    error: failure::Error,
    definition: Option<Definition>,
}

impl ConfigError {
    fn new(message: String, definition: Definition) -> ConfigError {
        ConfigError {
            error: failure::err_msg(message),
            definition: Some(definition),
        }
    }

    fn expected(key: &str, expected: &str, found: &ConfigValue) -> ConfigError {
        ConfigError {
            error: failure::format_err!(
                "`{}` expected {}, but found a {}",
                key,
                expected,
                found.desc()
            ),
            definition: Some(Definition::Path(found.definition_path().to_path_buf())),
        }
    }

    fn missing(key: &str) -> ConfigError {
        ConfigError {
            error: failure::format_err!("missing config key `{}`", key),
            definition: None,
        }
    }

    fn with_key_context(self, key: &str, definition: Definition) -> ConfigError {
        ConfigError {
            error: failure::format_err!("could not load config key `{}`: {}", key, self),
            definition: Some(definition),
        }
    }
}

impl std::error::Error for ConfigError {
}

// Future note: currently, we cannot override `Fail::cause` (due to
// specialization) so we have no way to return the underlying causes. In the
// future, once this limitation is lifted, this should instead implement
// `cause` and avoid doing the cause formatting here.
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = self
            .error
            .iter_chain()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\nCaused by:\n  ");
        if let Some(ref definition) = self.definition {
            write!(f, "error in {}: {}", definition, message)
        } else {
            message.fmt(f)
        }
    }
}

impl de::Error for ConfigError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        ConfigError {
            error: failure::err_msg(msg.to_string()),
            definition: None,
        }
    }
}

impl From<failure::Error> for ConfigError {
    fn from(error: failure::Error) -> Self {
        ConfigError {
            error,
            definition: None,
        }
    }
}

/// Serde deserializer used to convert config values to a target type using
/// `Config::get`.
pub struct Deserializer<'config> {
    config: &'config Config,
    key: ConfigKey,
}

macro_rules! deserialize_method {
    ($method:ident, $visit:ident, $getter:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: de::Visitor<'de>,
        {
            let v = self.config.$getter(&self.key)?.ok_or_else(||
                ConfigError::missing(&self.key.to_config()))?;
            let Value{val, definition} = v;
            let res: Result<V::Value, ConfigError> = visitor.$visit(val);
            res.map_err(|e| e.with_key_context(&self.key.to_config(), definition))
        }
    }
}

impl<'de, 'config> de::Deserializer<'de> for Deserializer<'config> {
    type Error = ConfigError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        // Future note: If you ever need to deserialize a non-self describing
        // map type, this should implement a starts_with check (similar to how
        // ConfigMapAccess does).
        if let Some(v) = self.config.env.get(&self.key.to_env()) {
            let res: Result<V::Value, ConfigError> = if v == "true" || v == "false" {
                visitor.visit_bool(v.parse().unwrap())
            } else if let Ok(v) = v.parse::<i64>() {
                visitor.visit_i64(v)
            } else if self.config.cli_unstable().advanced_env
                && v.starts_with('[')
                && v.ends_with(']')
            {
                visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?)
            } else {
                visitor.visit_string(v.clone())
            };
            return res.map_err(|e| {
                e.with_key_context(
                    &self.key.to_config(),
                    Definition::Environment(self.key.to_env()),
                )
            });
        }

        let o_cv = self.config.get_cv(&self.key.to_config())?;
        if let Some(cv) = o_cv {
            let res: (Result<V::Value, ConfigError>, PathBuf) = match cv {
                CV::Integer(i, path) => (visitor.visit_i64(i), path),
                CV::String(s, path) => (visitor.visit_string(s), path),
                CV::List(_, path) => (
                    visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?),
                    path,
                ),
                CV::Table(_, path) => (
                    visitor.visit_map(ConfigMapAccess::new_map(self.config, self.key.clone())?),
                    path,
                ),
                CV::Boolean(b, path) => (visitor.visit_bool(b), path),
            };
            let (res, path) = res;
            return res
                .map_err(|e| e.with_key_context(&self.key.to_config(), Definition::Path(path)));
        }
        Err(ConfigError::missing(&self.key.to_config()))
    }

    deserialize_method!(deserialize_bool, visit_bool, get_bool_priv);
    deserialize_method!(deserialize_i8, visit_i64, get_integer);
    deserialize_method!(deserialize_i16, visit_i64, get_integer);
    deserialize_method!(deserialize_i32, visit_i64, get_integer);
    deserialize_method!(deserialize_i64, visit_i64, get_integer);
    deserialize_method!(deserialize_u8, visit_i64, get_integer);
    deserialize_method!(deserialize_u16, visit_i64, get_integer);
    deserialize_method!(deserialize_u32, visit_i64, get_integer);
    deserialize_method!(deserialize_u64, visit_i64, get_integer);
    deserialize_method!(deserialize_string, visit_string, get_string_priv);

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if self.config.has_key(&self.key) {
            visitor.visit_some(self)
        } else {
            // Treat missing values as `None`.
            visitor.visit_none()
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_map(ConfigMapAccess::new_struct(self.config, self.key, fields)?)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_map(ConfigMapAccess::new_map(self.config, self.key)?)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?)
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if name == "ConfigRelativePath" {
            match self.config.get_string_priv(&self.key)? {
                Some(v) => {
                    let path = v
                        .definition
                        .root(self.config)
                        .join(v.val)
                        .display()
                        .to_string();
                    visitor.visit_newtype_struct(path.into_deserializer())
                }
                None => Err(ConfigError::missing(&self.key.to_config())),
            }
        } else {
            visitor.visit_newtype_struct(self)
        }
    }

    // These aren't really supported, yet.
    serde::forward_to_deserialize_any! {
        f32 f64 char str bytes
        byte_buf unit unit_struct
        enum identifier ignored_any
    }
}

struct ConfigMapAccess<'config> {
    config: &'config Config,
    key: ConfigKey,
    set_iter: <HashSet<ConfigKeyPart> as IntoIterator>::IntoIter,
    next: Option<ConfigKeyPart>,
}

impl<'config> ConfigMapAccess<'config> {
    fn new_map(
        config: &'config Config,
        key: ConfigKey,
    ) -> Result<ConfigMapAccess<'config>, ConfigError> {
        let mut set = HashSet::new();
        if let Some(mut v) = config.get_table(&key.to_config())? {
            // `v: Value<HashMap<String, CV>>`
            for (key, _value) in v.val.drain() {
                set.insert(ConfigKeyPart::CasePart(key));
            }
        }
        if config.cli_unstable().advanced_env {
            // `CARGO_PROFILE_DEV_OVERRIDES_`
            let env_pattern = format!("{}_", key.to_env());
            for env_key in config.env.keys() {
                if env_key.starts_with(&env_pattern) {
                    // `CARGO_PROFILE_DEV_OVERRIDES_bar_OPT_LEVEL = 3`
                    let rest = &env_key[env_pattern.len()..];
                    // `rest = bar_OPT_LEVEL`
                    let part = rest.splitn(2, '_').next().unwrap();
                    // `part = "bar"`
                    set.insert(ConfigKeyPart::CasePart(part.to_string()));
                }
            }
        }
        Ok(ConfigMapAccess {
            config,
            key,
            set_iter: set.into_iter(),
            next: None,
        })
    }

    fn new_struct(
        config: &'config Config,
        key: ConfigKey,
        fields: &'static [&'static str],
    ) -> Result<ConfigMapAccess<'config>, ConfigError> {
        let mut set = HashSet::new();
        for field in fields {
            set.insert(ConfigKeyPart::Part(field.to_string()));
        }
        if let Some(mut v) = config.get_table(&key.to_config())? {
            for (t_key, value) in v.val.drain() {
                let part = ConfigKeyPart::Part(t_key);
                if !set.contains(&part) {
                    config.shell().warn(format!(
                        "unused key `{}` in config file `{}`",
                        key.join(part).to_config(),
                        value.definition_path().display()
                    ))?;
                }
            }
        }
        Ok(ConfigMapAccess {
            config,
            key,
            set_iter: set.into_iter(),
            next: None,
        })
    }
}

impl<'de, 'config> de::MapAccess<'de> for ConfigMapAccess<'config> {
    type Error = ConfigError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.set_iter.next() {
            Some(key) => {
                let de_key = key.to_config();
                self.next = Some(key);
                seed.deserialize(de_key.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let next_key = self.next.take().expect("next field missing");
        let next_key = self.key.join(next_key);
        seed.deserialize(Deserializer {
            config: self.config,
            key: next_key,
        })
    }
}

struct ConfigSeqAccess {
    list_iter: vec::IntoIter<(String, Definition)>,
}

impl ConfigSeqAccess {
    fn new(config: &Config, key: &ConfigKey) -> Result<ConfigSeqAccess, ConfigError> {
        let mut res = Vec::new();
        if let Some(v) = config.get_list(&key.to_config())? {
            for (s, path) in v.val {
                res.push((s, Definition::Path(path)));
            }
        }

        if config.cli_unstable().advanced_env {
            // Parse an environment string as a TOML array.
            let env_key = key.to_env();
            let def = Definition::Environment(env_key.clone());
            if let Some(v) = config.env.get(&env_key) {
                if !(v.starts_with('[') && v.ends_with(']')) {
                    return Err(ConfigError::new(
                        format!("should have TOML list syntax, found `{}`", v),
                        def,
                    ));
                }
                let temp_key = key.last().to_env();
                let toml_s = format!("{}={}", temp_key, v);
                let toml_v: toml::Value = toml::de::from_str(&toml_s).map_err(|e| {
                    ConfigError::new(format!("could not parse TOML list: {}", e), def.clone())
                })?;
                let values = toml_v
                    .as_table()
                    .unwrap()
                    .get(&temp_key)
                    .unwrap()
                    .as_array()
                    .expect("env var was not array");
                for value in values {
                    // TODO: support other types.
                    let s = value.as_str().ok_or_else(|| {
                        ConfigError::new(
                            format!("expected string, found {}", value.type_str()),
                            def.clone(),
                        )
                    })?;
                    res.push((s.to_string(), def.clone()));
                }
            }
        }
        Ok(ConfigSeqAccess {
            list_iter: res.into_iter(),
        })
    }
}

impl<'de> de::SeqAccess<'de> for ConfigSeqAccess {
    type Error = ConfigError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.list_iter.next() {
            // TODO: add `def` to error?
            Some((value, _def)) => seed.deserialize(value.into_deserializer()).map(Some),
            None => Ok(None),
        }
    }
}

/// Use with the `get` API to fetch a string that will be converted to a
/// `PathBuf`. Relative paths are converted to absolute paths based on the
/// location of the config file.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct ConfigRelativePath(PathBuf);

impl ConfigRelativePath {
    pub fn path(self) -> PathBuf {
        self.0
    }
}

#[derive(Eq, PartialEq, Clone)]
pub enum ConfigValue {
    Integer(i64, PathBuf),
    String(String, PathBuf),
    List(Vec<(String, PathBuf)>, PathBuf),
    Table(HashMap<String, ConfigValue>, PathBuf),
    Boolean(bool, PathBuf),
}

pub struct Value<T> {
    pub val: T,
    pub definition: Definition,
}

pub type OptValue<T> = Option<Value<T>>;

#[derive(Clone, Debug)]
pub enum Definition {
    Path(PathBuf),
    Environment(String),
}

impl fmt::Debug for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            CV::Integer(i, ref path) => write!(f, "{} (from {})", i, path.display()),
            CV::Boolean(b, ref path) => write!(f, "{} (from {})", b, path.display()),
            CV::String(ref s, ref path) => write!(f, "{} (from {})", s, path.display()),
            CV::List(ref list, ref path) => {
                write!(f, "[")?;
                for (i, &(ref s, ref path)) in list.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} (from {})", s, path.display())?;
                }
                write!(f, "] (from {})", path.display())
            }
            CV::Table(ref table, _) => write!(f, "{:?}", table),
        }
    }
}

impl ConfigValue {
    fn from_toml(path: &Path, toml: toml::Value) -> CargoResult<ConfigValue> {
        match toml {
            toml::Value::String(val) => Ok(CV::String(val, path.to_path_buf())),
            toml::Value::Boolean(b) => Ok(CV::Boolean(b, path.to_path_buf())),
            toml::Value::Integer(i) => Ok(CV::Integer(i, path.to_path_buf())),
            toml::Value::Array(val) => Ok(CV::List(
                val.into_iter()
                    .map(|toml| match toml {
                        toml::Value::String(val) => Ok((val, path.to_path_buf())),
                        v => failure::bail!("expected string but found {} in list", v.type_str()),
                    })
                    .collect::<CargoResult<_>>()?,
                path.to_path_buf(),
            )),
            toml::Value::Table(val) => Ok(CV::Table(
                val.into_iter()
                    .map(|(key, value)| {
                        let value = CV::from_toml(path, value)
                            .chain_err(|| format!("failed to parse key `{}`", key))?;
                        Ok((key, value))
                    })
                    .collect::<CargoResult<_>>()?,
                path.to_path_buf(),
            )),
            v => failure::bail!(
                "found TOML configuration value of unknown type `{}`",
                v.type_str()
            ),
        }
    }

    fn into_toml(self) -> toml::Value {
        match self {
            CV::Boolean(s, _) => toml::Value::Boolean(s),
            CV::String(s, _) => toml::Value::String(s),
            CV::Integer(i, _) => toml::Value::Integer(i),
            CV::List(l, _) => {
                toml::Value::Array(l.into_iter().map(|(s, _)| toml::Value::String(s)).collect())
            }
            CV::Table(l, _) => {
                toml::Value::Table(l.into_iter().map(|(k, v)| (k, v.into_toml())).collect())
            }
        }
    }

    fn merge(&mut self, from: ConfigValue) -> CargoResult<()> {
        match (self, from) {
            (&mut CV::List(ref mut old, _), CV::List(ref mut new, _)) => {
                let new = mem::replace(new, Vec::new());
                old.extend(new.into_iter());
            }
            (&mut CV::Table(ref mut old, _), CV::Table(ref mut new, _)) => {
                let new = mem::replace(new, HashMap::new());
                for (key, value) in new {
                    match old.entry(key.clone()) {
                        Occupied(mut entry) => {
                            let path = value.definition_path().to_path_buf();
                            let entry = entry.get_mut();
                            entry.merge(value).chain_err(|| {
                                format!(
                                    "failed to merge key `{}` between \
                                     files:\n  \
                                     file 1: {}\n  \
                                     file 2: {}",
                                    key,
                                    entry.definition_path().display(),
                                    path.display()
                                )
                            })?;
                        }
                        Vacant(entry) => {
                            entry.insert(value);
                        }
                    };
                }
            }
            // Allow switching types except for tables or arrays.
            (expected @ &mut CV::List(_, _), found)
            | (expected @ &mut CV::Table(_, _), found)
            | (expected, found @ CV::List(_, _))
            | (expected, found @ CV::Table(_, _)) => {
                return Err(internal(format!(
                    "expected {}, but found {}",
                    expected.desc(),
                    found.desc()
                )));
            }
            _ => {}
        }

        Ok(())
    }

    pub fn i64(&self, key: &str) -> CargoResult<(i64, &Path)> {
        match *self {
            CV::Integer(i, ref p) => Ok((i, p)),
            _ => self.expected("integer", key),
        }
    }

    pub fn string(&self, key: &str) -> CargoResult<(&str, &Path)> {
        match *self {
            CV::String(ref s, ref p) => Ok((s, p)),
            _ => self.expected("string", key),
        }
    }

    pub fn table(&self, key: &str) -> CargoResult<(&HashMap<String, ConfigValue>, &Path)> {
        match *self {
            CV::Table(ref table, ref p) => Ok((table, p)),
            _ => self.expected("table", key),
        }
    }

    pub fn list(&self, key: &str) -> CargoResult<&[(String, PathBuf)]> {
        match *self {
            CV::List(ref list, _) => Ok(list),
            _ => self.expected("list", key),
        }
    }

    pub fn boolean(&self, key: &str) -> CargoResult<(bool, &Path)> {
        match *self {
            CV::Boolean(b, ref p) => Ok((b, p)),
            _ => self.expected("bool", key),
        }
    }

    pub fn desc(&self) -> &'static str {
        match *self {
            CV::Table(..) => "table",
            CV::List(..) => "array",
            CV::String(..) => "string",
            CV::Boolean(..) => "boolean",
            CV::Integer(..) => "integer",
        }
    }

    pub fn definition_path(&self) -> &Path {
        match *self {
            CV::Boolean(_, ref p)
            | CV::Integer(_, ref p)
            | CV::String(_, ref p)
            | CV::List(_, ref p)
            | CV::Table(_, ref p) => p,
        }
    }

    fn expected<T>(&self, wanted: &str, key: &str) -> CargoResult<T> {
        failure::bail!(
            "expected a {}, but found a {} for `{}` in {}",
            wanted,
            self.desc(),
            key,
            self.definition_path().display()
        )
    }
}

impl Definition {
    pub fn root<'a>(&'a self, config: &'a Config) -> &'a Path {
        match *self {
            Definition::Path(ref p) => p.parent().unwrap().parent().unwrap(),
            Definition::Environment(_) => config.cwd(),
        }
    }
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Definition::Path(ref p) => p.display().fmt(f),
            Definition::Environment(ref key) => write!(f, "environment variable `{}`", key),
        }
    }
}

pub fn homedir(cwd: &Path) -> Option<PathBuf> {
    ::home::cargo_home_with_cwd(cwd).ok()
}

fn walk_tree<F>(pwd: &Path, home: &Path, mut walk: F) -> CargoResult<()>
where
    F: FnMut(&Path) -> CargoResult<()>,
{
    let mut stash: HashSet<PathBuf> = HashSet::new();

    for current in paths::ancestors(pwd) {
        let possible = current.join(".cargo").join("config");
        if fs::metadata(&possible).is_ok() {
            walk(&possible)?;
            stash.insert(possible);
        }
    }

    // Once we're done, also be sure to walk the home directory even if it's not
    // in our history to be sure we pick up that standard location for
    // information.
    let config = home.join("config");
    if !stash.contains(&config) && fs::metadata(&config).is_ok() {
        walk(&config)?;
    }

    Ok(())
}

pub fn save_credentials(cfg: &Config, token: String, registry: Option<String>) -> CargoResult<()> {
    let mut file = {
        cfg.home_path.create_dir()?;
        cfg.home_path
            .open_rw(Path::new("credentials"), cfg, "credentials' config file")?
    };

    let (key, value) = {
        let key = "token".to_string();
        let value = ConfigValue::String(token, file.path().to_path_buf());
        let mut map = HashMap::new();
        map.insert(key, value);
        let table = CV::Table(map, file.path().to_path_buf());

        if let Some(registry) = registry {
            let mut map = HashMap::new();
            map.insert(registry, table);
            (
                "registries".into(),
                CV::Table(map, file.path().to_path_buf()),
            )
        } else {
            ("registry".into(), table)
        }
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents).chain_err(|| {
        format!(
            "failed to read configuration file `{}`",
            file.path().display()
        )
    })?;

    let mut toml = cargo_toml::parse(&contents, file.path(), cfg)?;

    // Move the old token location to the new one.
    if let Some(token) = toml.as_table_mut().unwrap().remove("token") {
        let mut map = HashMap::new();
        map.insert("token".to_string(), token);
        toml.as_table_mut()
            .unwrap()
            .insert("registry".into(), map.into());
    }

    toml.as_table_mut().unwrap().insert(key, value.into_toml());

    let contents = toml.to_string();
    file.seek(SeekFrom::Start(0))?;
    file.write_all(contents.as_bytes())?;
    file.file().set_len(contents.len() as u64)?;
    set_permissions(file.file(), 0o600)?;

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
