use std::{fmt, os, mem};
use std::cell::{RefCell, RefMut};
use std::collections::hash_map::{HashMap};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::io;
use std::io::fs::{mod, PathExtensions, File};
use std::string;

use rustc_serialize::{Encodable,Encoder};
use toml;
use core::MultiShell;
use ops;
use util::{CargoResult, ChainError, Require, internal, human};

use util::toml as cargo_toml;

use self::ConfigValue as CV;

pub struct Config<'a> {
    home_path: Path,
    shell: RefCell<&'a mut MultiShell>,
    jobs: uint,
    target: Option<string::String>,
    rustc_version: string::String,
    /// The current host and default target of rustc
    rustc_host: string::String,
}

impl<'a> Config<'a> {
    pub fn new(shell: &'a mut MultiShell,
               jobs: Option<uint>,
               target: Option<string::String>) -> CargoResult<Config<'a>> {
        if jobs == Some(0) {
            return Err(human("jobs must be at least 1"))
        }

        let (rustc_version, rustc_host) = try!(ops::rustc_version());

        Ok(Config {
            home_path: try!(homedir().require(|| {
                human("Cargo couldn't find your home directory. \
                      This probably means that $HOME was not set.")
            })),
            shell: RefCell::new(shell),
            jobs: jobs.unwrap_or(os::num_cpus()),
            target: target,
            rustc_version: rustc_version,
            rustc_host: rustc_host,
        })
    }

    pub fn home(&self) -> &Path { &self.home_path }

    pub fn git_db_path(&self) -> Path {
        self.home_path.join(".cargo").join("git").join("db")
    }

    pub fn git_checkout_path(&self) -> Path {
        self.home_path.join(".cargo").join("git").join("checkouts")
    }

    pub fn registry_index_path(&self) -> Path {
        self.home_path.join(".cargo").join("registry").join("index")
    }

    pub fn registry_cache_path(&self) -> Path {
        self.home_path.join(".cargo").join("registry").join("cache")
    }

    pub fn registry_source_path(&self) -> Path {
        self.home_path.join(".cargo").join("registry").join("src")
    }

    pub fn shell(&self) -> RefMut<&'a mut MultiShell> {
        self.shell.borrow_mut()
    }

    pub fn jobs(&self) -> uint {
        self.jobs
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_ref().map(|t| t.as_slice())
    }

    /// Return the output of `rustc -v verbose`
    pub fn rustc_version(&self) -> &str {
        self.rustc_version.as_slice()
    }

    /// Return the host platform and default target of rustc
    pub fn rustc_host(&self) -> &str {
        self.rustc_host.as_slice()
    }
}

#[deriving(Eq, PartialEq, Clone, RustcEncodable, RustcDecodable, Copy)]
pub enum Location {
    Project,
    Global
}

#[deriving(Eq,PartialEq,Clone,RustcDecodable)]
pub enum ConfigValue {
    String(string::String, Path),
    List(Vec<(string::String, Path)>),
    Table(HashMap<string::String, ConfigValue>),
    Boolean(bool, Path),
}

impl fmt::Show for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CV::String(ref string, ref path) => {
                write!(f, "{} (from {})", string, path.display())
            }
            CV::List(ref list) => {
                try!(write!(f, "["));
                for (i, &(ref s, ref path)) in list.iter().enumerate() {
                    if i > 0 { try!(write!(f, ", ")); }
                    try!(write!(f, "{} (from {})", s, path.display()));
                }
                write!(f, "]")
            }
            CV::Table(ref table) => write!(f, "{}", table),
            CV::Boolean(b, ref path) => {
                write!(f, "{} (from {})", b, path.display())
            }
        }
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for ConfigValue {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        match *self {
            CV::String(ref string, _) => string.encode(s),
            CV::List(ref list) => {
                let list: Vec<&string::String> = list.iter().map(|s| &s.0).collect();
                list.encode(s)
            }
            CV::Table(ref table) => table.encode(s),
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
                        _ => Err(internal("")),
                    }
                }).collect::<CargoResult<_>>())))
            }
            toml::Value::Table(val) => {
                Ok(CV::Table(try!(val.into_iter().map(|(key, value)| {
                    let value = raw_try!(CV::from_toml(path, value));
                    Ok((key, value))
                }).collect::<CargoResult<_>>())))
            }
            _ => return Err(internal(""))
        }
    }

    fn merge(&mut self, from: ConfigValue) -> CargoResult<()> {
        match (self, from) {
            (&CV::String(..), CV::String(..)) |
            (&CV::Boolean(..), CV::Boolean(..)) => {}
            (&CV::List(ref mut old), CV::List(ref mut new)) => {
                let new = mem::replace(new, Vec::new());
                old.extend(new.into_iter());
            }
            (&CV::Table(ref mut old), CV::Table(ref mut new)) => {
                let new = mem::replace(new, HashMap::new());
                for (key, value) in new.into_iter() {
                    match old.entry(key) {
                        Occupied(mut entry) => { try!(entry.get_mut().merge(value)); }
                        Vacant(entry) => { entry.set(value); }
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
            _ => Err(internal(format!("expected a string, but found a {}",
                                      self.desc()))),
        }
    }

    pub fn table(&self) -> CargoResult<&HashMap<string::String, ConfigValue>> {
        match *self {
            CV::Table(ref table) => Ok(table),
            _ => Err(internal(format!("expected a table, but found a {}",
                                      self.desc()))),
        }
    }

    pub fn list(&self) -> CargoResult<&[(string::String, Path)]> {
        match *self {
            CV::List(ref list) => Ok(list.as_slice()),
            _ => Err(internal(format!("expected a list, but found a {}",
                                      self.desc()))),
        }
    }

    pub fn boolean(&self) -> CargoResult<(bool, &Path)> {
        match *self {
            CV::Boolean(b, ref p) => Ok((b, p)),
            _ => Err(internal(format!("expected a bool, but found a {}",
                                      self.desc()))),
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

    fn into_toml(self) -> toml::Value {
        match self {
            CV::Boolean(s, _) => toml::Value::Boolean(s),
            CV::String(s, _) => toml::Value::String(s),
            CV::List(l) => toml::Value::Array(l
                                        .into_iter()
                                        .map(|(s, _)| toml::Value::String(s))
                                        .collect()),
            CV::Table(l) => toml::Value::Table(l.into_iter()
                                        .map(|(k, v)| (k, v.into_toml()))
                                        .collect()),
        }
    }
}

fn homedir() -> Option<Path> {
    let cargo_home = os::getenv("CARGO_HOME").map(|p| Path::new(p));
    let user_home = os::homedir();
    return cargo_home.or(user_home);
}

pub fn get_config(pwd: Path, key: &str) -> CargoResult<ConfigValue> {
    find_in_tree(&pwd, |file| extract_config(file, key)).map_err(|_|
        human(format!("`{}` not found in your configuration", key)))
}

pub fn all_configs(pwd: Path) -> CargoResult<HashMap<string::String, ConfigValue>> {
    let mut cfg = CV::Table(HashMap::new());

    try!(walk_tree(&pwd, |mut file| {
        let path = file.path().clone();
        let contents = try!(file.read_to_string());
        let table = try!(cargo_toml::parse(contents.as_slice(), &path).chain_error(|| {
            internal(format!("could not parse Toml manifest; path={}",
                             path.display()))
        }));
        let value = try!(CV::from_toml(&path, toml::Value::Table(table)));
        try!(cfg.merge(value));
        Ok(())
    }).chain_error(|| human("Couldn't load Cargo configuration")));


    match cfg {
        CV::Table(map) => Ok(map),
        _ => unreachable!(),
    }
}

fn find_in_tree<T>(pwd: &Path,
                   walk: |File| -> CargoResult<T>) -> CargoResult<T> {
    let mut current = pwd.clone();

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let file = try!(File::open(&possible));

            match walk(file) {
                Ok(res) => return Ok(res),
                _ => ()
            }
        }

        if !current.pop() { break; }
    }

    Err(internal(""))
}

fn walk_tree(pwd: &Path,
             walk: |File| -> CargoResult<()>) -> CargoResult<()> {
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
    let home = try!(homedir().require(|| {
        human("Cargo couldn't find your home directory. \
              This probably means that $HOME was not set.")
    }));
    if !home.is_ancestor_of(pwd) {
        let config = home.join(".cargo/config");
        if config.exists() {
            let file = try!(File::open(&config));
            try!(walk(file));
        }
    }

    Ok(())
}

fn extract_config(mut file: File, key: &str) -> CargoResult<ConfigValue> {
    let contents = try!(file.read_to_string());
    let mut toml = try!(cargo_toml::parse(contents.as_slice(), file.path()));
    let val = try!(toml.remove(&key.to_string()).require(|| internal("")));

    CV::from_toml(file.path(), val)
}

pub fn set_config(cfg: &Config, loc: Location, key: &str,
                  value: ConfigValue) -> CargoResult<()> {
    // TODO: There are a number of drawbacks here
    //
    // 1. Project is unimplemented
    // 2. This blows away all comments in a file
    // 3. This blows away the previous ordering of a file.
    let file = match loc {
        Location::Global => cfg.home_path.join(".cargo").join("config"),
        Location::Project => unimplemented!(),
    };
    try!(fs::mkdir_recursive(&file.dir_path(), io::USER_DIR));
    let contents = File::open(&file).read_to_string().unwrap_or("".to_string());
    let mut toml = try!(cargo_toml::parse(contents.as_slice(), &file));
    toml.insert(key.to_string(), value.into_toml());
    try!(File::create(&file).write(toml::Value::Table(toml).to_string().as_bytes()));
    Ok(())
}
