use std::{fmt, os, mem};
use std::cell::{RefCell, RefMut, Ref, Cell};
use std::collections::hash_map::{HashMap};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::io;
use std::io::fs::{self, PathExtensions, File};

use rustc_serialize::{Encodable,Encoder};
use toml;
use core::MultiShell;
use ops;
use util::{CargoResult, ChainError, internal, human};

use util::toml as cargo_toml;

use self::ConfigValue as CV;

pub struct Config<'a> {
    home_path: Path,
    shell: RefCell<&'a mut MultiShell>,
    rustc_version: String,
    /// The current host and default target of rustc
    rustc_host: String,
    values: RefCell<HashMap<String, ConfigValue>>,
    values_loaded: Cell<bool>,
    cwd: Path,
}

impl<'a> Config<'a> {
    pub fn new(shell: &'a mut MultiShell) -> CargoResult<Config<'a>> {
        let cwd = try!(os::getcwd().chain_error(|| {
            human("couldn't get the current directory of the process")
        }));
        let (rustc_version, rustc_host) = try!(ops::rustc_version());

        Ok(Config {
            home_path: try!(homedir().chain_error(|| {
                human("Cargo couldn't find your home directory. \
                      This probably means that $HOME was not set.")
            })),
            shell: RefCell::new(shell),
            rustc_version: rustc_version,
            rustc_host: rustc_host,
            cwd: cwd,
            values: RefCell::new(HashMap::new()),
            values_loaded: Cell::new(false),
        })
    }

    pub fn home(&self) -> &Path { &self.home_path }

    pub fn git_db_path(&self) -> Path {
        self.home_path.join("git").join("db")
    }

    pub fn git_checkout_path(&self) -> Path {
        self.home_path.join("git").join("checkouts")
    }

    pub fn registry_index_path(&self) -> Path {
        self.home_path.join("registry").join("index")
    }

    pub fn registry_cache_path(&self) -> Path {
        self.home_path.join("registry").join("cache")
    }

    pub fn registry_source_path(&self) -> Path {
        self.home_path.join("registry").join("src")
    }

    pub fn shell(&self) -> RefMut<&'a mut MultiShell> {
        self.shell.borrow_mut()
    }

    /// Return the output of `rustc -v verbose`
    pub fn rustc_version(&self) -> &str {
        self.rustc_version.as_slice()
    }

    /// Return the host platform and default target of rustc
    pub fn rustc_host(&self) -> &str {
        self.rustc_host.as_slice()
    }

    pub fn values(&self) -> CargoResult<Ref<HashMap<String, ConfigValue>>> {
        if !self.values_loaded.get() {
            try!(self.load_values());
            self.values_loaded.set(true);
        }
        Ok(self.values.borrow())
    }

    pub fn cwd(&self) -> &Path { &self.cwd }

    pub fn get(&self, key: &str) -> CargoResult<Option<ConfigValue>> {
        let vals = try!(self.values());
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
                CV::String(_, ref path) |
                CV::List(_, ref path) |
                CV::Boolean(_, ref path) => {
                    let idx = key.split('.').take(i)
                                 .fold(0, |n, s| n + s.len()) + i - 1;
                    let key_so_far = key.slice_to(idx);
                    return Err(human(format!("expected table for configuration \
                                              key `{}`, but found {} in {:?}",
                                             key_so_far, val.desc(), path)));
                }
            }
        }
        Ok(Some(val.clone()))
    }

    pub fn get_string(&self, key: &str) -> CargoResult<Option<(String, Path)>> {
        match try!(self.get(key)) {
            Some(CV::String(i, path)) => Ok(Some((i, path))),
            Some(val) => self.expected("string", key, val),
            None => Ok(None),
        }
    }

    pub fn get_table(&self, key: &str)
                    -> CargoResult<Option<(HashMap<String, CV>, Path)>> {
        match try!(self.get(key)) {
            Some(CV::Table(i, path)) => Ok(Some((i, path))),
            Some(val) => self.expected("table", key, val),
            None => Ok(None),
        }
    }

    pub fn expected<T>(&self, ty: &str, key: &str, val: CV) -> CargoResult<T> {
        val.expected(ty).map_err(|e| {
            human(format!("invalid configuration for key `{}`\n{}", key, e))
        })
    }

    fn load_values(&self) -> CargoResult<()> {
        let mut cfg = CV::Table(HashMap::new(), Path::new("."));

        try!(walk_tree(&self.cwd, |mut file| {
            let path = file.path().clone();
            let contents = try!(file.read_to_string());
            let table = try!(cargo_toml::parse(contents.as_slice(),
                                               &path).chain_error(|| {
                human(format!("could not parse TOML configuration in `{:?}`",
                              path))
            }));
            let toml = toml::Value::Table(table);
            let value = try!(CV::from_toml(&path, toml).chain_error(|| {
                human(format!("failed to load TOML configuration from `{:?}`",
                              path))
            }));
            try!(cfg.merge(value));
            Ok(())
        }).chain_error(|| human("Couldn't load Cargo configuration")));


        *self.values.borrow_mut() = match cfg {
            CV::Table(map, _) => map,
            _ => unreachable!(),
        };
        Ok(())
    }
}

#[derive(Eq, PartialEq, Clone, RustcEncodable, RustcDecodable, Copy)]
pub enum Location {
    Project,
    Global
}

#[derive(Eq,PartialEq,Clone,RustcDecodable)]
pub enum ConfigValue {
    String(String, Path),
    List(Vec<(String, Path)>, Path),
    Table(HashMap<String, ConfigValue>, Path),
    Boolean(bool, Path),
}

impl fmt::Show for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CV::String(ref string, ref path) => {
                write!(f, "{} (from {})", string, path.display())
            }
            CV::List(ref list, ref path) => {
                try!(write!(f, "["));
                for (i, &(ref s, ref path)) in list.iter().enumerate() {
                    if i > 0 { try!(write!(f, ", ")); }
                    try!(write!(f, "{} (from {})", s, path.display()));
                }
                write!(f, "] (from {:?})", path)
            }
            CV::Table(ref table, _) => write!(f, "{:?}", table),
            CV::Boolean(b, ref path) => {
                write!(f, "{} (from {})", b, path.display())
            }
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
        }
    }
}

impl ConfigValue {
    fn from_toml(path: &Path, toml: toml::Value) -> CargoResult<ConfigValue> {
        match toml {
            toml::Value::String(val) => Ok(CV::String(val, path.clone())),
            toml::Value::Boolean(b) => Ok(CV::Boolean(b, path.clone())),
            toml::Value::Array(val) => {
                Ok(CV::List(try!(val.into_iter().map(|toml| {
                    match toml {
                        toml::Value::String(val) => Ok((val, path.clone())),
                        v => Err(human(format!("expected string but found {} \
                                                in list", v.type_str()))),
                    }
                }).collect::<CargoResult<_>>()), path.clone()))
            }
            toml::Value::Table(val) => {
                Ok(CV::Table(try!(val.into_iter().map(|(key, value)| {
                    let value = try!(CV::from_toml(path, value).chain_error(|| {
                        human(format!("failed to parse key `{}`", key))
                    }));
                    Ok((key, value))
                }).collect::<CargoResult<_>>()), path.clone()))
            }
            v => return Err(human(format!("found TOML configuration value of \
                                           unknown type `{}`", v.type_str())))
        }
    }

    fn merge(&mut self, from: ConfigValue) -> CargoResult<()> {
        match (self, from) {
            (&mut CV::String(..), CV::String(..)) |
            (&mut CV::Boolean(..), CV::Boolean(..)) => {}
            (&mut CV::List(ref mut old, _), CV::List(ref mut new, _)) => {
                let new = mem::replace(new, Vec::new());
                old.extend(new.into_iter());
            }
            (&mut CV::Table(ref mut old, _), CV::Table(ref mut new, _)) => {
                let new = mem::replace(new, HashMap::new());
                for (key, value) in new.into_iter() {
                    match old.entry(key) {
                        Occupied(mut entry) => { try!(entry.get_mut().merge(value)); }
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

    pub fn string(&self) -> CargoResult<(&str, &Path)> {
        match *self {
            CV::String(ref s, ref p) => Ok((s.as_slice(), p)),
            _ => self.expected("string"),
        }
    }

    pub fn table(&self) -> CargoResult<(&HashMap<String, ConfigValue>, &Path)> {
        match *self {
            CV::Table(ref table, ref p) => Ok((table, p)),
            _ => self.expected("table"),
        }
    }

    pub fn list(&self) -> CargoResult<&[(String, Path)]> {
        match *self {
            CV::List(ref list, _) => Ok(list.as_slice()),
            _ => self.expected("list"),
        }
    }

    pub fn boolean(&self) -> CargoResult<(bool, &Path)> {
        match *self {
            CV::Boolean(b, ref p) => Ok((b, p)),
            _ => self.expected("bool"),
        }
    }

    pub fn desc(&self) -> &'static str {
        match *self {
            CV::Table(..) => "table",
            CV::List(..) => "array",
            CV::String(..) => "string",
            CV::Boolean(..) => "boolean",
        }
    }

    pub fn definition_path(&self) -> &Path {
        match *self  {
            CV::Boolean(_, ref p) |
            CV::String(_, ref p) |
            CV::List(_, ref p) |
            CV::Table(_, ref p) => p
        }
    }

    fn expected<T>(&self, wanted: &str) -> CargoResult<T> {
        Err(internal(format!("expected a {}, but found a {} in {:?}",
                             wanted, self.desc(), self.definition_path())))
    }

    fn into_toml(self) -> toml::Value {
        match self {
            CV::Boolean(s, _) => toml::Value::Boolean(s),
            CV::String(s, _) => toml::Value::String(s),
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

fn homedir() -> Option<Path> {
    let cargo_home = os::getenv("CARGO_HOME").map(|p| Path::new(p));
    let user_home = os::homedir().map(|p| p.join(".cargo"));
    return cargo_home.or(user_home);
}

fn walk_tree<F>(pwd: &Path, mut walk: F) -> CargoResult<()>
    where F: FnMut(File) -> CargoResult<()>
{
    let mut current = pwd.clone();

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let file = try!(File::open(&possible));

            try!(walk(file));
        }
        if !current.pop() { break; }
    }

    // Once we're done, also be sure to walk the home directory even if it's not
    // in our history to be sure we pick up that standard location for
    // information.
    let home = try!(homedir().chain_error(|| {
        human("Cargo couldn't find your home directory. \
              This probably means that $HOME was not set.")
    }));
    if !home.is_ancestor_of(pwd) {
        let config = home.join("config");
        if config.exists() {
            let file = try!(File::open(&config));
            try!(walk(file));
        }
    }

    Ok(())
}

pub fn set_config(cfg: &Config, loc: Location, key: &str,
                  value: ConfigValue) -> CargoResult<()> {
    // TODO: There are a number of drawbacks here
    //
    // 1. Project is unimplemented
    // 2. This blows away all comments in a file
    // 3. This blows away the previous ordering of a file.
    let file = match loc {
        Location::Global => cfg.home_path.join("config"),
        Location::Project => unimplemented!(),
    };
    try!(fs::mkdir_recursive(&file.dir_path(), io::USER_DIR));
    let contents = File::open(&file).read_to_string().unwrap_or("".to_string());
    let mut toml = try!(cargo_toml::parse(contents.as_slice(), &file));
    toml.insert(key.to_string(), value.into_toml());
    try!(File::create(&file).write(toml::Value::Table(toml).to_string().as_bytes()));
    Ok(())
}
