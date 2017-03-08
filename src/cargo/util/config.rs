use std::cell::{RefCell, RefMut, Cell};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_map::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs::{self, File};
use std::io::SeekFrom;
use std::io::prelude::*;
use std::mem;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rustc_serialize::{Encodable,Encoder};
use toml;
use core::shell::{Verbosity, ColorConfig};
use core::MultiShell;
use util::{CargoResult, CargoError, ChainError, Rustc, internal, human};
use util::{Filesystem, LazyCell};

use util::toml as cargo_toml;

use self::ConfigValue as CV;

pub struct Config {
    home_path: Filesystem,
    shell: RefCell<MultiShell>,
    rustc: LazyCell<Rustc>,
    values: LazyCell<HashMap<String, ConfigValue>>,
    cwd: PathBuf,
    cargo_exe: LazyCell<PathBuf>,
    rustdoc: LazyCell<PathBuf>,
    extra_verbose: Cell<bool>,
    frozen: Cell<bool>,
    locked: Cell<bool>,
}

impl Config {
    pub fn new(shell: MultiShell,
               cwd: PathBuf,
               homedir: PathBuf) -> Config {
        Config {
            home_path: Filesystem::new(homedir),
            shell: RefCell::new(shell),
            rustc: LazyCell::new(),
            cwd: cwd,
            values: LazyCell::new(),
            cargo_exe: LazyCell::new(),
            rustdoc: LazyCell::new(),
            extra_verbose: Cell::new(false),
            frozen: Cell::new(false),
            locked: Cell::new(false),
        }
    }

    pub fn default() -> CargoResult<Config> {
        let shell = ::shell(Verbosity::Verbose, ColorConfig::Auto);
        let cwd = env::current_dir().chain_error(|| {
            human("couldn't get the current directory of the process")
        })?;
        let homedir = homedir(&cwd).chain_error(|| {
            human("Cargo couldn't find your home directory. \
                  This probably means that $HOME was not set.")
        })?;
        Ok(Config::new(shell, cwd, homedir))
    }

    pub fn home(&self) -> &Filesystem { &self.home_path }

    pub fn git_path(&self) -> Filesystem {
        self.home_path.join("git")
    }

    pub fn registry_index_path(&self) -> Filesystem {
        self.home_path.join("registry").join("index")
    }

    pub fn registry_cache_path(&self) -> Filesystem {
        self.home_path.join("registry").join("cache")
    }

    pub fn registry_source_path(&self) -> Filesystem {
        self.home_path.join("registry").join("src")
    }

    pub fn shell(&self) -> RefMut<MultiShell> {
        self.shell.borrow_mut()
    }

    pub fn rustdoc(&self) -> CargoResult<&Path> {
        self.rustdoc.get_or_try_init(|| self.get_tool("rustdoc")).map(AsRef::as_ref)
    }

    pub fn rustc(&self) -> CargoResult<&Rustc> {
        self.rustc.get_or_try_init(|| Rustc::new(self.get_tool("rustc")?))
    }

    pub fn cargo_exe(&self) -> CargoResult<&Path> {
        self.cargo_exe.get_or_try_init(||
            env::current_exe().and_then(|path| path.canonicalize())
            .chain_error(|| {
                human("couldn't get the path to cargo executable")
            })
        ).map(AsRef::as_ref)
    }

    pub fn values(&self) -> CargoResult<&HashMap<String, ConfigValue>> {
        self.values.get_or_try_init(|| self.load_values())
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
        where Box<CargoError>: From<V::Err>
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

    pub fn get_path(&self, key: &str) -> CargoResult<Option<Value<PathBuf>>> {
        if let Some(val) = self.get_string(key)? {
            let is_path = val.val.contains('/') ||
                          (cfg!(windows) && val.val.contains('\\'));
            let path = if is_path {
                val.definition.root(self).join(val.val)
            } else {
                // A pathless name
                PathBuf::from(val.val)
            };
            Ok(Some(Value {
                val: path,
                definition: val.definition,
            }))
        } else {
            Ok(None)
        }
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
            human(format!("invalid configuration for key `{}`\n{}", key, e))
        })
    }

    pub fn configure(&self,
                     verbose: u32,
                     quiet: Option<bool>,
                     color: &Option<String>,
                     frozen: bool,
                     locked: bool) -> CargoResult<()> {
        let extra_verbose = verbose >= 2;
        let verbose = if verbose == 0 {None} else {Some(true)};

        // Ignore errors in the configuration files.
        let cfg_verbose = self.get_bool("term.verbose").unwrap_or(None).map(|v| v.val);
        let cfg_color = self.get_string("term.color").unwrap_or(None).map(|v| v.val);

        let color = color.as_ref().or(cfg_color.as_ref());

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
        self.shell().set_color_config(color.map(|s| &s[..]))?;
        self.extra_verbose.set(extra_verbose);
        self.frozen.set(frozen);
        self.locked.set(locked);

        Ok(())
    }

    pub fn extra_verbose(&self) -> bool {
        self.extra_verbose.get()
    }

    pub fn network_allowed(&self) -> bool {
        !self.frozen.get()
    }

    pub fn lock_update_allowed(&self) -> bool {
        !self.frozen.get() && !self.locked.get()
    }

    fn load_values(&self) -> CargoResult<HashMap<String, ConfigValue>> {
        let mut cfg = CV::Table(HashMap::new(), PathBuf::from("."));

        walk_tree(&self.cwd, |mut file, path| {
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            let toml = cargo_toml::parse(&contents,
                                         &path,
                                         self).chain_error(|| {
                human(format!("could not parse TOML configuration in `{}`",
                              path.display()))
            })?;
            let value = CV::from_toml(&path, toml).chain_error(|| {
                human(format!("failed to load TOML configuration from `{}`",
                              path.display()))
            })?;
            cfg.merge(value)?;
            Ok(())
        }).chain_error(|| human("Couldn't load Cargo configuration"))?;


        match cfg {
            CV::Table(map, _) => Ok(map),
            _ => unreachable!(),
        }
    }

    fn get_tool(&self, tool: &str) -> CargoResult<PathBuf> {
        let var = tool.chars().flat_map(|c| c.to_uppercase()).collect::<String>();
        if let Some(tool_path) = env::var_os(&var) {
            return Ok(PathBuf::from(tool_path));
        }

        let var = format!("build.{}", tool);
        if let Some(tool_path) = self.get_path(&var)? {
            return Ok(tool_path.val);
        }

        Ok(PathBuf::from(tool))
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

impl Encodable for ConfigValue {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        match *self {
            CV::String(ref string, _) => string.encode(s),
            CV::List(ref list, _) => {
                let list: Vec<&String> = list.iter().map(|s| &s.0).collect();
                list.encode(s)
            }
            CV::Table(ref table, _) => table.encode(s),
            CV::Boolean(b, _) => b.encode(s),
            CV::Integer(i, _) => i.encode(s),
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
                        v => Err(human(format!("expected string but found {} \
                                                in list", v.type_str()))),
                    }
                }).collect::<CargoResult<_>>()?, path.to_path_buf()))
            }
            toml::Value::Table(val) => {
                Ok(CV::Table(val.into_iter().map(|(key, value)| {
                    let value = CV::from_toml(path, value).chain_error(|| {
                        human(format!("failed to parse key `{}`", key))
                    })?;
                    Ok((key, value))
                }).collect::<CargoResult<_>>()?, path.to_path_buf()))
            }
            v => bail!("found TOML configuration value of unknown type `{}`",
                       v.type_str()),
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
                for (key, value) in new.into_iter() {
                    match old.entry(key.clone()) {
                        Occupied(mut entry) => {
                            let path = value.definition_path().to_path_buf();
                            let entry = entry.get_mut();
                            entry.merge(value).chain_error(|| {
                                human(format!("failed to merge key `{}` between \
                                               files:\n  \
                                               file 1: {}\n  \
                                               file 2: {}",
                                              key,
                                              entry.definition_path().display(),
                                              path.display()))

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

    fn expected<T>(&self, wanted: &str, key: &str) -> CargoResult<T> {
        Err(human(format!("expected a {}, but found a {} for `{}` in {}",
                          wanted, self.desc(), key,
                          self.definition_path().display())))
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
    let cargo_home = env::var_os("CARGO_HOME").map(|home| {
        cwd.join(home)
    });
    if cargo_home.is_some() {
        return cargo_home
    }

    // If `CARGO_HOME` wasn't defined then we want to fall back to
    // `$HOME/.cargo`. Note that currently, however, the implementation of
    // `env::home_dir()` uses the $HOME environment variable *on all platforms*.
    // Platforms like Windows then have *another* fallback based on system APIs
    // if this isn't set.
    //
    // Specifically on Windows this can lead to some weird behavior where if you
    // invoke cargo inside an MSYS shell it'll have $HOME defined and it'll
    // place output there by default. If, however, you run in another shell
    // (like cmd.exe or powershell) it'll place output in
    // `C:\Users\$user\.cargo` by default.
    //
    // This snippet is meant to handle this case to ensure that on Windows we
    // always place output in the same location, regardless of the shell we were
    // invoked from. We first check `env::home_dir()` without tampering the
    // environment, and then afterwards we remove `$HOME` and call it again to
    // see what happened. If they both returned success then on Windows we only
    // return the first (with the $HOME in place) if it already exists. This
    // should help existing installs of Cargo continue using the same cargo home
    // directory.
    let home_dir_with_env = env::home_dir().map(|p| p.join(".cargo"));
    let home_dir = env::var_os("HOME");
    env::remove_var("HOME");
    let home_dir_without_env = env::home_dir().map(|p| p.join(".cargo"));
    if let Some(home_dir) = home_dir {
        env::set_var("HOME", home_dir);
    }

    match (home_dir_with_env, home_dir_without_env) {
        (None, None) => None,
        (None, Some(p)) |
        (Some(p), None) => Some(p),
        (Some(a), Some(b)) => {
            if cfg!(windows) && !a.exists() {
                Some(b)
            } else {
                Some(a)
            }
        }
    }
}

fn walk_tree<F>(pwd: &Path, mut walk: F) -> CargoResult<()>
    where F: FnMut(File, &Path) -> CargoResult<()>
{
    let mut current = pwd;
    let mut stash: HashSet<PathBuf> = HashSet::new();

    loop {
        let possible = current.join(".cargo").join("config");
        if fs::metadata(&possible).is_ok() {
            let file = File::open(&possible)?;

            walk(file, &possible)?;

            stash.insert(possible);
        }

        match current.parent() {
            Some(p) => current = p,
            None => break,
        }
    }

    // Once we're done, also be sure to walk the home directory even if it's not
    // in our history to be sure we pick up that standard location for
    // information.
    let home = homedir(pwd).chain_error(|| {
        human("Cargo couldn't find your home directory. \
              This probably means that $HOME was not set.")
    })?;
    let config = home.join("config");
    if !stash.contains(&config) && fs::metadata(&config).is_ok() {
        let file = File::open(&config)?;
        walk(file, &config)?;
    }

    Ok(())
}

pub fn set_config(cfg: &Config,
                  loc: Location,
                  key: &str,
                  value: ConfigValue) -> CargoResult<()> {
    // TODO: There are a number of drawbacks here
    //
    // 1. Project is unimplemented
    // 2. This blows away all comments in a file
    // 3. This blows away the previous ordering of a file.
    let mut file = match loc {
        Location::Global => {
            cfg.home_path.create_dir()?;
            cfg.home_path.open_rw(Path::new("config"), cfg,
                                       "the global config file")?
        }
        Location::Project => unimplemented!(),
    };
    let mut contents = String::new();
    let _ = file.read_to_string(&mut contents);
    let mut toml = cargo_toml::parse(&contents, file.path(), cfg)?;
    toml.as_table_mut()
        .unwrap()
        .insert(key.to_string(), value.into_toml());

    let contents = toml.to_string();
    file.seek(SeekFrom::Start(0))?;
    file.write_all(contents.as_bytes())?;
    file.file().set_len(contents.len() as u64)?;
    Ok(())
}
