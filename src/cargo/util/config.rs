use std::cell::{RefCell, RefMut};
use std::collections::HashSet;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_map::HashMap;
use std::env;
use std::fmt;
use std::fs::{self, File};
use std::io::SeekFrom;
use std::io::prelude::*;
use std::mem;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Once, ONCE_INIT};

use curl::easy::Easy;
use jobserver;
use serde::{Serialize, Serializer};
use toml;

use core::shell::Verbosity;
use core::{Shell, CliUnstable};
use ops;
use url::Url;
use util::ToUrl;
use util::Rustc;
use util::errors::{CargoResult, CargoResultExt, CargoError, internal};
use util::paths;
use util::toml as cargo_toml;
use util::{Filesystem, LazyCell};

use self::ConfigValue as CV;

/// Configuration information for cargo. This is not specific to a build, it is information
/// relating to cargo itself.
///
/// This struct implements `Default`: all fields can be inferred.
#[derive(Debug)]
pub struct Config {
    /// The location of the users's 'home' directory. OS-dependent.
    home_path: Filesystem,
    /// Information about how to write messages to the shell
    shell: RefCell<Shell>,
    /// Information on how to invoke the compiler (rustc)
    rustc: LazyCell<Rustc>,
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
}

impl Config {
    pub fn new(shell: Shell,
               cwd: PathBuf,
               homedir: PathBuf) -> Config {
        static mut GLOBAL_JOBSERVER: *mut jobserver::Client = 0 as *mut _;
        static INIT: Once = ONCE_INIT;

        // This should be called early on in the process, so in theory the
        // unsafety is ok here. (taken ownership of random fds)
        INIT.call_once(|| unsafe {
            if let Some(client) = jobserver::Client::from_env() {
                GLOBAL_JOBSERVER = Box::into_raw(Box::new(client));
            }
        });

        Config {
            home_path: Filesystem::new(homedir),
            shell: RefCell::new(shell),
            rustc: LazyCell::new(),
            cwd: cwd,
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
        }
    }

    pub fn default() -> CargoResult<Config> {
        let shell = Shell::new();
        let cwd = env::current_dir().chain_err(|| {
            "couldn't get the current directory of the process"
        })?;
        let homedir = homedir(&cwd).ok_or_else(|| {
            "Cargo couldn't find your home directory. \
             This probably means that $HOME was not set."
        })?;
        Ok(Config::new(shell, cwd, homedir))
    }

    /// The user's cargo home directory (OS-dependent)
    pub fn home(&self) -> &Filesystem { &self.home_path }

    /// The cargo git directory (`<cargo_home>/git`)
    pub fn git_path(&self) -> Filesystem {
        self.home_path.join("git")
    }

    /// The cargo registry index directory (`<cargo_home>/registry/index`)
    pub fn registry_index_path(&self) -> Filesystem {
        self.home_path.join("registry").join("index")
    }

    /// The cargo registry cache directory (`<cargo_home>/registry/path`)
    pub fn registry_cache_path(&self) -> Filesystem {
        self.home_path.join("registry").join("cache")
    }

    /// The cargo registry source directory (`<cargo_home>/registry/src`)
    pub fn registry_source_path(&self) -> Filesystem {
        self.home_path.join("registry").join("src")
    }

    /// Get a reference to the shell, for e.g. writing error messages
    pub fn shell(&self) -> RefMut<Shell> {
        self.shell.borrow_mut()
    }

    /// Get the path to the `rustdoc` executable
    pub fn rustdoc(&self) -> CargoResult<&Path> {
        self.rustdoc.get_or_try_init(|| self.get_tool("rustdoc")).map(AsRef::as_ref)
    }

    /// Get the path to the `rustc` executable
    pub fn rustc(&self) -> CargoResult<&Rustc> {
        self.rustc.get_or_try_init(|| Rustc::new(self.get_tool("rustc")?,
                                                 self.maybe_get_tool("rustc_wrapper")?))
    }

    /// Get the path to the `cargo` executable
    pub fn cargo_exe(&self) -> CargoResult<&Path> {
        self.cargo_exe.get_or_try_init(|| {
            fn from_current_exe() -> CargoResult<PathBuf> {
                // Try fetching the path to `cargo` using env::current_exe().
                // The method varies per operating system and might fail; in particular,
                // it depends on /proc being mounted on Linux, and some environments
                // (like containers or chroots) may not have that available.
                env::current_exe()
                    .and_then(|path| path.canonicalize())
                    .map_err(CargoError::from)
            }

            fn from_argv() -> CargoResult<PathBuf> {
                // Grab argv[0] and attempt to resolve it to an absolute path.
                // If argv[0] has one component, it must have come from a PATH lookup,
                // so probe PATH in that case.
                // Otherwise, it has multiple components and is either:
                // - a relative path (e.g. `./cargo`, `target/debug/cargo`), or
                // - an absolute path (e.g. `/usr/local/bin/cargo`).
                // In either case, Path::canonicalize will return the full absolute path
                // to the target if it exists
                env::args_os()
                    .next()
                    .ok_or(CargoError::from("no argv[0]"))
                    .map(PathBuf::from)
                    .and_then(|argv0| {
                        if argv0.components().count() == 1 {
                            probe_path(argv0)
                        } else {
                            argv0.canonicalize().map_err(CargoError::from)
                        }
                    })
            }

            fn probe_path(argv0: PathBuf) -> CargoResult<PathBuf> {
                let paths = env::var_os("PATH").ok_or(CargoError::from("no PATH"))?;
                for path in env::split_paths(&paths) {
                    let candidate = PathBuf::from(path).join(&argv0);
                    if candidate.is_file() {
                        // PATH may have a component like "." in it, so we still need to
                        // canonicalize.
                        return candidate.canonicalize().map_err(CargoError::from);
                    }
                }

                Err(CargoError::from("no cargo executable candidate found in PATH"))
            }

            from_current_exe()
                .or_else(|_| from_argv())
                .chain_err(|| "couldn't get the path to cargo executable")
        }).map(AsRef::as_ref)
    }

    pub fn values(&self) -> CargoResult<&HashMap<String, ConfigValue>> {
        self.values.get_or_try_init(|| self.load_values())
    }

    pub fn set_values(&self, values: HashMap<String, ConfigValue>) -> CargoResult<()> {
        if self.values.borrow().is_some() {
            return Err("Config values already found".into());
        }
        match self.values.fill(values) {
            Ok(()) => Ok(()),
            Err(_) => Err("Could not fill values".into()),
        }
    }

    pub fn cwd(&self) -> &Path { &self.cwd }

    pub fn target_dir(&self) -> CargoResult<Option<Filesystem>> {
        if let Some(dir) = env::var_os("CARGO_TARGET_DIR") {
            Ok(Some(Filesystem::new(self.cwd.join(dir))))
        } else if let Some(val) = self.get_path("build.target-dir")? {
            let val = self.cwd.join(val.val);
            Ok(Some(Filesystem::new(val)))
        } else {
            Ok(None)
        }
    }

    fn get(&self, key: &str) -> CargoResult<Option<ConfigValue>> {
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
                CV::Integer(_, ref path) |
                CV::String(_, ref path) |
                CV::List(_, ref path) |
                CV::Boolean(_, ref path) => {
                    let idx = key.split('.').take(i)
                                 .fold(0, |n, s| n + s.len()) + i - 1;
                    let key_so_far = &key[..idx];
                    bail!("expected table for configuration key `{}`, \
                           but found {} in {}",
                          key_so_far, val.desc(), path.display())
                }
            }
        }
        Ok(Some(val.clone()))
    }

    fn get_env<V: FromStr>(&self, key: &str) -> CargoResult<Option<Value<V>>>
        where CargoError: From<V::Err>
    {
        let key = key.replace(".", "_")
                     .replace("-", "_")
                     .chars()
                     .flat_map(|c| c.to_uppercase())
                     .collect::<String>();
        match env::var(&format!("CARGO_{}", key)) {
            Ok(value) => {
                Ok(Some(Value {
                    val: value.parse()?,
                    definition: Definition::Environment,
                }))
            }
            Err(..) => Ok(None),
        }
    }

    pub fn get_string(&self, key: &str) -> CargoResult<Option<Value<String>>> {
        if let Some(v) = self.get_env(key)? {
            return Ok(Some(v))
        }
        match self.get(key)? {
            Some(CV::String(i, path)) => {
                Ok(Some(Value {
                    val: i,
                    definition: Definition::Path(path),
                }))
            }
            Some(val) => self.expected("string", key, val),
            None => Ok(None),
        }
    }

    pub fn get_bool(&self, key: &str) -> CargoResult<Option<Value<bool>>> {
        if let Some(v) = self.get_env(key)? {
            return Ok(Some(v))
        }
        match self.get(key)? {
            Some(CV::Boolean(b, path)) => {
                Ok(Some(Value {
                    val: b,
                    definition: Definition::Path(path),
                }))
            }
            Some(val) => self.expected("bool", key, val),
            None => Ok(None),
        }
    }

    fn string_to_path(&self, value: String, definition: &Definition) -> PathBuf {
        let is_path = value.contains('/') ||
                      (cfg!(windows) && value.contains('\\'));
        if is_path {
            definition.root(self).join(value)
        } else {
            // A pathless name
            PathBuf::from(value)
        }
    }

    pub fn get_path(&self, key: &str) -> CargoResult<Option<Value<PathBuf>>> {
        if let Some(val) = self.get_string(key)? {
            Ok(Some(Value {
                val: self.string_to_path(val.val, &val.definition),
                definition: val.definition
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_path_and_args(&self, key: &str)
                             -> CargoResult<Option<Value<(PathBuf, Vec<String>)>>> {
        if let Some(mut val) = self.get_list_or_split_string(key)? {
            if !val.val.is_empty() {
                return Ok(Some(Value {
                    val: (self.string_to_path(val.val.remove(0), &val.definition), val.val),
                    definition: val.definition
                }));
            }
        }
        Ok(None)
    }

    pub fn get_list(&self, key: &str)
                    -> CargoResult<Option<Value<Vec<(String, PathBuf)>>>> {
        match self.get(key)? {
            Some(CV::List(i, path)) => {
                Ok(Some(Value {
                    val: i,
                    definition: Definition::Path(path),
                }))
            }
            Some(val) => self.expected("list", key, val),
            None => Ok(None),
        }
    }

    pub fn get_list_or_split_string(&self, key: &str)
                    -> CargoResult<Option<Value<Vec<String>>>> {
        match self.get_env::<String>(key) {
            Ok(Some(value)) =>
                return Ok(Some(Value {
                    val: value.val.split(' ').map(str::to_string).collect(),
                    definition: value.definition
                })),
            Err(err) => return Err(err),
            Ok(None) => (),
        }

        match self.get(key)? {
            Some(CV::List(i, path)) => {
                Ok(Some(Value {
                    val: i.into_iter().map(|(s, _)| s).collect(),
                    definition: Definition::Path(path),
                }))
            }
            Some(CV::String(i, path)) => {
                Ok(Some(Value {
                    val: i.split(' ').map(str::to_string).collect(),
                    definition: Definition::Path(path),
                }))
            }
            Some(val) => self.expected("list or string", key, val),
            None => Ok(None),
        }
    }

    pub fn get_table(&self, key: &str)
                    -> CargoResult<Option<Value<HashMap<String, CV>>>> {
        match self.get(key)? {
            Some(CV::Table(i, path)) => {
                Ok(Some(Value {
                    val: i,
                    definition: Definition::Path(path),
                }))
            }
            Some(val) => self.expected("table", key, val),
            None => Ok(None),
        }
    }

    pub fn get_i64(&self, key: &str) -> CargoResult<Option<Value<i64>>> {
        if let Some(v) = self.get_env(key)? {
            return Ok(Some(v))
        }
        match self.get(key)? {
            Some(CV::Integer(i, path)) => {
                Ok(Some(Value {
                    val: i,
                    definition: Definition::Path(path),
                }))
            }
            Some(val) => self.expected("integer", key, val),
            None => Ok(None),
        }
    }

    pub fn net_retry(&self) -> CargoResult<i64> {
        match self.get_i64("net.retry")? {
            Some(v) => {
                let value = v.val;
                if value < 0 {
                    bail!("net.retry must be positive, but found {} in {}",
                      v.val, v.definition)
                } else {
                    Ok(value)
                }
            }
            None => Ok(2),
        }
    }

    pub fn expected<T>(&self, ty: &str, key: &str, val: CV) -> CargoResult<T> {
        val.expected(ty, key).map_err(|e| {
            format!("invalid configuration for key `{}`\n{}", key, e).into()
        })
    }

    pub fn configure(&mut self,
                     verbose: u32,
                     quiet: Option<bool>,
                     color: &Option<String>,
                     frozen: bool,
                     locked: bool,
                     unstable_flags: &[String]) -> CargoResult<()> {
        let extra_verbose = verbose >= 2;
        let verbose = if verbose == 0 {None} else {Some(true)};

        // Ignore errors in the configuration files.
        let cfg_verbose = self.get_bool("term.verbose").unwrap_or(None).map(|v| v.val);
        let cfg_color = self.get_string("term.color").unwrap_or(None).map(|v| v.val);

        let color = color.as_ref().or_else(|| cfg_color.as_ref());

        let verbosity = match (verbose, cfg_verbose, quiet) {
            (Some(true), _, None) |
            (None, Some(true), None) => Verbosity::Verbose,

            // command line takes precedence over configuration, so ignore the
            // configuration.
            (None, _, Some(true)) => Verbosity::Quiet,

            // Can't pass both at the same time on the command line regardless
            // of configuration.
            (Some(true), _, Some(true)) => {
                bail!("cannot set both --verbose and --quiet");
            }

            // Can't actually get `Some(false)` as a value from the command
            // line, so just ignore them here to appease exhaustiveness checking
            // in match statements.
            (Some(false), _, _) |
            (_, _, Some(false)) |

            (None, Some(false), None) |
            (None, None, None) => Verbosity::Normal,
        };

        self.shell().set_verbosity(verbosity);
        self.shell().set_color_choice(color.map(|s| &s[..]))?;
        self.extra_verbose = extra_verbose;
        self.frozen = frozen;
        self.locked = locked;
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
        !self.frozen
    }

    pub fn lock_update_allowed(&self) -> bool {
        !self.frozen && !self.locked
    }

    /// Loads configuration from the filesystem
    pub fn load_values(&self) -> CargoResult<HashMap<String, ConfigValue>> {
        let mut cfg = CV::Table(HashMap::new(), PathBuf::from("."));

        walk_tree(&self.cwd, |path| {
            let mut contents = String::new();
            let mut file = File::open(&path)?;
            file.read_to_string(&mut contents).chain_err(|| {
                format!("failed to read configuration file `{}`",
                              path.display())
            })?;
            let toml = cargo_toml::parse(&contents,
                                         path,
                                         self).chain_err(|| {
                format!("could not parse TOML configuration in `{}`",
                        path.display())
            })?;
            let value = CV::from_toml(path, toml).chain_err(|| {
                format!("failed to load TOML configuration from `{}`",
                        path.display())
            })?;
            cfg.merge(value).chain_err(|| {
                format!("failed to merge configuration at `{}`", path.display())
            })?;
            Ok(())
        }).chain_err(|| "Couldn't load Cargo configuration")?;

        self.load_credentials(&mut cfg)?;
        match cfg {
            CV::Table(map, _) => Ok(map),
            _ => unreachable!(),
        }
    }

    /// Gets the index for a registry.
    pub fn get_registry_index(&self, registry: &str) -> CargoResult<Url> {
        Ok(match self.get_string(&format!("registries.{}.index", registry))? {
            Some(index) => index.val.to_url()?,
            None => return Err(CargoError::from(format!("No index found for registry: `{}`", registry)).into()),
        })
    }

    /// Loads credentials config from the credentials file into the ConfigValue object, if present.
    fn load_credentials(&self, cfg: &mut ConfigValue) -> CargoResult<()> {
        let home_path = self.home_path.clone().into_path_unlocked();
        let credentials = home_path.join("credentials");
        if !fs::metadata(&credentials).is_ok() {
            return Ok(());
        }

        let mut contents = String::new();
        let mut file = File::open(&credentials)?;
        file.read_to_string(&mut contents).chain_err(|| {
            format!("failed to read configuration file `{}`", credentials.display())
        })?;

        let toml = cargo_toml::parse(&contents,
                                     &credentials,
                                     self).chain_err(|| {
            format!("could not parse TOML configuration in `{}`", credentials.display())
        })?;

        let value = CV::from_toml(&credentials, toml).chain_err(|| {
            format!("failed to load TOML configuration from `{}`", credentials.display())
        })?;

        let cfg = match *cfg {
            CV::Table(ref mut map, _) => map,
            _ => unreachable!(),
        };

        let registry = cfg.entry("registry".into())
                          .or_insert_with(|| CV::Table(HashMap::new(), PathBuf::from(".")));

        match (registry, value) {
            (&mut CV::Table(ref mut old, _), CV::Table(ref mut new, _)) => {
                // Take ownership of `new` by swapping it with an empty hashmap, so we can move
                // into an iterator.
                let new = mem::replace(new, HashMap::new());
                for (key, value) in new {
                    old.insert(key, value);
                }
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    /// Look for a path for `tool` in an environment variable or config path, but return `None`
    /// if it's not present.
    fn maybe_get_tool(&self, tool: &str) -> CargoResult<Option<PathBuf>> {
        let var = tool.chars().flat_map(|c| c.to_uppercase()).collect::<String>();
        if let Some(tool_path) = env::var_os(&var) {
            return Ok(Some(PathBuf::from(tool_path)));
        }

        let var = format!("build.{}", tool);
        if let Some(tool_path) = self.get_path(&var)? {
            return Ok(Some(tool_path.val));
        }

        Ok(None)
    }

    /// Look for a path for `tool` in an environment variable or config path, defaulting to `tool`
    /// as a path.
    fn get_tool(&self, tool: &str) -> CargoResult<PathBuf> {
        self.maybe_get_tool(tool)
            .map(|t| t.unwrap_or_else(|| PathBuf::from(tool)))
    }

    pub fn jobserver_from_env(&self) -> Option<&jobserver::Client> {
        self.jobserver.as_ref()
    }

    pub fn http(&self) -> CargoResult<&RefCell<Easy>> {
        self.easy.get_or_try_init(|| {
            ops::http_handle(self).map(RefCell::new)
        })
    }
}

#[derive(Eq, PartialEq, Clone, Copy)]
pub enum Location {
    Project,
    Global
}

#[derive(Eq,PartialEq,Clone,Deserialize)]
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

pub enum Definition {
    Path(PathBuf),
    Environment,
}

impl fmt::Debug for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CV::Integer(i, ref path) => write!(f, "{} (from {})", i,
                                               path.display()),
            CV::Boolean(b, ref path) => write!(f, "{} (from {})", b,
                                               path.display()),
            CV::String(ref s, ref path) => write!(f, "{} (from {})", s,
                                                  path.display()),
            CV::List(ref list, ref path) => {
                write!(f, "[")?;
                for (i, &(ref s, ref path)) in list.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{} (from {})", s, path.display())?;
                }
                write!(f, "] (from {})", path.display())
            }
            CV::Table(ref table, _) => write!(f, "{:?}", table),
        }
    }
}

impl Serialize for ConfigValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            CV::String(ref string, _) => string.serialize(s),
            CV::List(ref list, _) => {
                let list: Vec<&String> = list.iter().map(|s| &s.0).collect();
                list.serialize(s)
            }
            CV::Table(ref table, _) => table.serialize(s),
            CV::Boolean(b, _) => b.serialize(s),
            CV::Integer(i, _) => i.serialize(s),
        }
    }
}

impl ConfigValue {
    fn from_toml(path: &Path, toml: toml::Value) -> CargoResult<ConfigValue> {
        match toml {
            toml::Value::String(val) => Ok(CV::String(val, path.to_path_buf())),
            toml::Value::Boolean(b) => Ok(CV::Boolean(b, path.to_path_buf())),
            toml::Value::Integer(i) => Ok(CV::Integer(i, path.to_path_buf())),
            toml::Value::Array(val) => {
                Ok(CV::List(val.into_iter().map(|toml| {
                    match toml {
                        toml::Value::String(val) => Ok((val, path.to_path_buf())),
                        v => Err(format!("expected string but found {} \
                                                in list", v.type_str()).into()),
                    }
                }).collect::<CargoResult<_>>()?, path.to_path_buf()))
            }
            toml::Value::Table(val) => {
                Ok(CV::Table(val.into_iter().map(|(key, value)| {
                    let value = CV::from_toml(path, value).chain_err(|| {
                        format!("failed to parse key `{}`", key)
                    })?;
                    Ok((key, value))
                }).collect::<CargoResult<_>>()?, path.to_path_buf()))
            }
            v => bail!("found TOML configuration value of unknown type `{}`",
                       v.type_str()),
        }
    }

    fn into_toml(self) -> toml::Value {
        match self {
            CV::Boolean(s, _) => toml::Value::Boolean(s),
            CV::String(s, _) => toml::Value::String(s),
            CV::Integer(i, _) => toml::Value::Integer(i),
            CV::List(l, _) => toml::Value::Array(l
                                          .into_iter()
                                          .map(|(s, _)| toml::Value::String(s))
                                          .collect()),
            CV::Table(l, _) => toml::Value::Table(l.into_iter()
                                          .map(|(k, v)| (k, v.into_toml()))
                                          .collect()),
        }
    }

    fn merge(&mut self, from: ConfigValue) -> CargoResult<()> {
        match (self, from) {
            (&mut CV::String(..), CV::String(..)) |
            (&mut CV::Integer(..), CV::Integer(..)) |
            (&mut CV::Boolean(..), CV::Boolean(..)) => {}
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
                                format!("failed to merge key `{}` between \
                                         files:\n  \
                                         file 1: {}\n  \
                                         file 2: {}",
                                        key,
                                        entry.definition_path().display(),
                                        path.display())

                            })?;
                        }
                        Vacant(entry) => { entry.insert(value); }
                    };
                }
            }
            (expected, found) => {
                return Err(internal(format!("expected {}, but found {}",
                                            expected.desc(), found.desc())))
            }
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

    pub fn table(&self, key: &str)
                 -> CargoResult<(&HashMap<String, ConfigValue>, &Path)> {
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
        match *self  {
            CV::Boolean(_, ref p) |
            CV::Integer(_, ref p) |
            CV::String(_, ref p) |
            CV::List(_, ref p) |
            CV::Table(_, ref p) => p
        }
    }

    pub fn expected<T>(&self, wanted: &str, key: &str) -> CargoResult<T> {
        Err(format!("expected a {}, but found a {} for `{}` in {}",
                    wanted, self.desc(), key,
                    self.definition_path().display()).into())
    }
}

impl Definition {
    pub fn root<'a>(&'a self, config: &'a Config) -> &'a Path {
        match *self {
            Definition::Path(ref p) => p.parent().unwrap().parent().unwrap(),
            Definition::Environment => config.cwd(),
        }
    }
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Definition::Path(ref p) => p.display().fmt(f),
            Definition::Environment => "the environment".fmt(f),
        }
    }
}

pub fn homedir(cwd: &Path) -> Option<PathBuf> {
    ::home::cargo_home_with_cwd(cwd).ok()
}

fn walk_tree<F>(pwd: &Path, mut walk: F) -> CargoResult<()>
    where F: FnMut(&Path) -> CargoResult<()>
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
    let home = homedir(pwd).ok_or_else(|| {
        CargoError::from("Cargo couldn't find your home directory. \
                          This probably means that $HOME was not set.")
    })?;
    let config = home.join("config");
    if !stash.contains(&config) && fs::metadata(&config).is_ok() {
        walk(&config)?;
    }

    Ok(())
}

pub fn save_credentials(cfg: &Config,
                        token: String,
                        registry: Option<String>) -> CargoResult<()> {
    let mut file = {
        cfg.home_path.create_dir()?;
        cfg.home_path.open_rw(Path::new("credentials"), cfg,
                              "credentials' config file")?
    };

    let (key, value) = {
        let key = "token".to_string();
        let value = ConfigValue::String(token, file.path().to_path_buf());

        if let Some(registry) = registry {
            let mut map = HashMap::new();
            map.insert(key, value);
            (registry, CV::Table(map, file.path().to_path_buf()))
        } else {
            (key, value)
        }
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents).chain_err(|| {
        format!("failed to read configuration file `{}`", file.path().display())
    })?;

    let mut toml = cargo_toml::parse(&contents, file.path(), cfg)?;
    toml.as_table_mut()
        .unwrap()
        .insert(key, value.into_toml());

    let contents = toml.to_string();
    file.seek(SeekFrom::Start(0))?;
    file.write_all(contents.as_bytes())?;
    file.file().set_len(contents.len() as u64)?;
    set_permissions(file.file(), 0o600)?;

    return Ok(());

    #[cfg(unix)]
    fn set_permissions(file: & File, mode: u32) -> CargoResult<()> {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = file.metadata()?.permissions();
        perms.set_mode(mode);
        file.set_permissions(perms)?;
        Ok(())
    }

    #[cfg(not(unix))]
    #[allow(unused)]
    fn set_permissions(file: & File, mode: u32) -> CargoResult<()> {
        Ok(())
    }
}
