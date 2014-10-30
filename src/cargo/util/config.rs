use std::{fmt, os, mem};
use std::io::fs::{PathExtensions, File};
use std::collections::HashMap;
use std::collections::hashmap::{Occupied, Vacant};
use std::string;

use serialize::{Encodable,Encoder};
use toml;
use core::MultiShell;
use ops;
use util::{CargoResult, ChainError, Require, internal, human};

use util::toml as cargo_toml;

pub struct Config<'a> {
    home_path: Path,
    shell: &'a mut MultiShell<'a>,
    jobs: uint,
    target: Option<string::String>,
    linker: Option<string::String>,
    ar: Option<string::String>,
    rustc_version: string::String,
    /// The current host and default target of rustc
    rustc_host: string::String,
}

impl<'a> Config<'a> {
    pub fn new<'a>(shell: &'a mut MultiShell,
                   jobs: Option<uint>,
                   target: Option<string::String>) -> CargoResult<Config<'a>> {
        if jobs == Some(0) {
            return Err(human("jobs must be at least 1"))
        }

        let (rustc_version, rustc_host) = try!(ops::rustc_version());

        Ok(Config {
            home_path: try!(os::homedir().require(|| {
                human("Cargo couldn't find your home directory. \
                      This probably means that $HOME was not set.")
            })),
            shell: shell,
            jobs: jobs.unwrap_or(os::num_cpus()),
            target: target,
            ar: None,
            linker: None,
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

    pub fn shell(&mut self) -> &mut MultiShell {
        &mut *self.shell
    }

    pub fn jobs(&self) -> uint {
        self.jobs
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_ref().map(|t| t.as_slice())
    }

    pub fn set_ar(&mut self, ar: string::String) { self.ar = Some(ar); }

    pub fn set_linker(&mut self, linker: string::String) { self.linker = Some(linker); }

    pub fn linker(&self) -> Option<&str> {
        self.linker.as_ref().map(|t| t.as_slice())
    }
    pub fn ar(&self) -> Option<&str> {
        self.ar.as_ref().map(|t| t.as_slice())
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

#[deriving(Eq,PartialEq,Clone,Encodable,Decodable)]
pub enum Location {
    Project,
    Global
}

#[deriving(Eq,PartialEq,Clone,Decodable)]
pub enum ConfigValue {
    String(string::String, Path),
    List(Vec<(string::String, Path)>),
    Table(HashMap<string::String, ConfigValue>),
    Boolean(bool, Path),
}

impl fmt::Show for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            String(ref string, ref path) => {
                write!(f, "{} (from {})", string, path.display())
            }
            List(ref list) => {
                try!(write!(f, "["));
                for (i, &(ref s, ref path)) in list.iter().enumerate() {
                    if i > 0 { try!(write!(f, ", ")); }
                    try!(write!(f, "{} (from {})", s, path.display()));
                }
                write!(f, "]")
            }
            Table(ref table) => write!(f, "{}", table),
            Boolean(b, ref path) => write!(f, "{} (from {})", b, path.display()),
        }
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for ConfigValue {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        match *self {
            String(ref string, _) => string.encode(s),
            List(ref list) => {
                let list: Vec<&string::String> = list.iter().map(|s| s.ref0()).collect();
                list.encode(s)
            }
            Table(ref table) => table.encode(s),
            Boolean(b, _) => b.encode(s),
        }
    }
}

impl ConfigValue {
    fn from_toml(path: &Path, toml: toml::Value) -> CargoResult<ConfigValue> {
        match toml {
            toml::String(val) => Ok(String(val, path.clone())),
            toml::Boolean(b) => Ok(Boolean(b, path.clone())),
            toml::Array(val) => {
                Ok(List(try!(val.into_iter().map(|toml| {
                    match toml {
                        toml::String(val) => Ok((val, path.clone())),
                        _ => Err(internal("")),
                    }
                }).collect::<CargoResult<_>>())))
            }
            toml::Table(val) => {
                Ok(Table(try!(val.into_iter().map(|(key, value)| {
                    let value = raw_try!(ConfigValue::from_toml(path, value));
                    Ok((key, value))
                }).collect::<CargoResult<_>>())))
            }
            _ => return Err(internal(""))
        }
    }

    fn merge(&mut self, from: ConfigValue) -> CargoResult<()> {
        match (self, from) {
            (&String(..), String(..)) |
            (&Boolean(..), Boolean(..)) => {}
            (&List(ref mut old), List(ref mut new)) => {
                let new = mem::replace(new, Vec::new());
                old.extend(new.into_iter());
            }
            (&Table(ref mut old), Table(ref mut new)) => {
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
            String(ref s, ref p) => Ok((s.as_slice(), p)),
            _ => Err(internal(format!("expected a string, but found a {}",
                                      self.desc()))),
        }
    }

    pub fn table(&self) -> CargoResult<&HashMap<string::String, ConfigValue>> {
        match *self {
            Table(ref table) => Ok(table),
            _ => Err(internal(format!("expected a table, but found a {}",
                                      self.desc()))),
        }
    }

    pub fn list(&self) -> CargoResult<&[(string::String, Path)]> {
        match *self {
            List(ref list) => Ok(list.as_slice()),
            _ => Err(internal(format!("expected a list, but found a {}",
                                      self.desc()))),
        }
    }

    pub fn boolean(&self) -> CargoResult<(bool, &Path)> {
        match *self {
            Boolean(b, ref p) => Ok((b, p)),
            _ => Err(internal(format!("expected a bool, but found a {}",
                                      self.desc()))),
        }
    }

    pub fn desc(&self) -> &'static str {
        match *self {
            Table(..) => "table",
            List(..) => "array",
            String(..) => "string",
            Boolean(..) => "boolean",
        }
    }

    fn into_toml(self) -> toml::Value {
        match self {
            Boolean(s, _) => toml::Boolean(s),
            String(s, _) => toml::String(s),
            List(l) => toml::Array(l.into_iter().map(|(s, _)| toml::String(s))
                                    .collect()),
            Table(l) => toml::Table(l.into_iter()
                                     .map(|(k, v)| (k, v.into_toml()))
                                     .collect()),
        }
    }
}

pub fn get_config(pwd: Path, key: &str) -> CargoResult<ConfigValue> {
    find_in_tree(&pwd, |file| extract_config(file, key)).map_err(|_|
        human(format!("`{}` not found in your configuration", key)))
}

pub fn all_configs(pwd: Path) -> CargoResult<HashMap<string::String, ConfigValue>> {
    let mut cfg = Table(HashMap::new());

    try!(walk_tree(&pwd, |mut file| {
        let path = file.path().clone();
        let contents = try!(file.read_to_string());
        let table = try!(cargo_toml::parse(contents.as_slice(), &path).chain_error(|| {
            internal(format!("could not parse Toml manifest; path={}",
                             path.display()))
        }));
        let value = try!(ConfigValue::from_toml(&path, toml::Table(table)));
        try!(cfg.merge(value));
        Ok(())
    }).chain_error(|| human("Couldn't load Cargo configuration")));


    match cfg {
        Table(map) => Ok(map),
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

    Ok(())
}

fn extract_config(mut file: File, key: &str) -> CargoResult<ConfigValue> {
    let contents = try!(file.read_to_string());
    let mut toml = try!(cargo_toml::parse(contents.as_slice(), file.path()));
    let val = try!(toml.pop(&key.to_string()).require(|| internal("")));

    ConfigValue::from_toml(file.path(), val)
}

pub fn set_config(cfg: &Config, loc: Location, key: &str,
                  value: ConfigValue) -> CargoResult<()> {
    // TODO: There are a number of drawbacks here
    //
    // 1. Project is unimplemented
    // 2. This blows away all comments in a file
    // 3. This blows away the previous ordering of a file.
    let file = match loc {
        Global => cfg.home_path.join(".cargo").join("config"),
        Project => unimplemented!(),
    };
    let contents = File::open(&file).read_to_string().unwrap_or("".to_string());
    let mut toml = try!(cargo_toml::parse(contents.as_slice(), &file));
    toml.insert(key.to_string(), value.into_toml());
    try!(File::create(&file).write(toml::Table(toml).to_string().as_bytes()));
    Ok(())
}
