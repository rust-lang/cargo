use std::{io, fmt, os, result, mem};
use std::collections::HashMap;
use serialize::{Encodable,Encoder};
use toml;
use core::MultiShell;
use util::{CargoResult, ChainError, Require, internal, human};

use cargo_toml = util::toml;

pub struct Config<'a> {
    home_path: Path,
    update_remotes: bool,
    shell: &'a mut MultiShell,
    jobs: uint,
    target: Option<String>,
    linker: Option<String>,
    ar: Option<String>,
}

impl<'a> Config<'a> {
    pub fn new<'a>(shell: &'a mut MultiShell,
                   update_remotes: bool,
                   jobs: Option<uint>,
                   target: Option<String>) -> CargoResult<Config<'a>> {
        if jobs == Some(0) {
            return Err(human("jobs must be at least 1"))
        }
        Ok(Config {
            home_path: try!(os::homedir().require(|| {
                human("Cargo couldn't find your home directory. \
                      This probably means that $HOME was not set.")
            })),
            update_remotes: update_remotes,
            shell: shell,
            jobs: jobs.unwrap_or(os::num_cpus()),
            target: target,
            ar: None,
            linker: None,
        })
    }

    pub fn git_db_path(&self) -> Path {
        self.home_path.join(".cargo").join("git").join("db")
    }

    pub fn git_checkout_path(&self) -> Path {
        self.home_path.join(".cargo").join("git").join("checkouts")
    }

    pub fn shell<'a>(&'a mut self) -> &'a mut MultiShell {
        &mut *self.shell
    }

    pub fn update_remotes(&mut self) -> bool {
        self.update_remotes
    }

    pub fn jobs(&self) -> uint {
        self.jobs
    }

    pub fn target<'a>(&'a self) -> Option<&'a str> {
        self.target.as_ref().map(|t| t.as_slice())
    }

    pub fn set_ar(&mut self, ar: String) { self.ar = Some(ar); }

    pub fn set_linker(&mut self, linker: String) { self.linker = Some(linker); }

    pub fn linker<'a>(&'a self) -> Option<&'a str> {
        self.linker.as_ref().map(|t| t.as_slice())
    }
    pub fn ar<'a>(&'a self) -> Option<&'a str> {
        self.ar.as_ref().map(|t| t.as_slice())
    }
}

#[deriving(Eq,PartialEq,Clone,Encodable,Decodable)]
pub enum Location {
    Project,
    Global
}

#[deriving(Eq,PartialEq,Clone,Decodable)]
pub enum ConfigValueValue {
    String(String),
    List(Vec<String>),
    Table(HashMap<String, ConfigValue>),
}

impl fmt::Show for ConfigValueValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            String(ref string) => write!(f, "{}", string),
            List(ref list) => write!(f, "{}", list),
            Table(ref table) => write!(f, "{}", table),
        }
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for ConfigValueValue {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        match *self {
            String(ref string) => string.encode(s),
            List(ref list) => list.encode(s),
            Table(ref table) => table.encode(s),
        }
    }
}

#[deriving(Eq,PartialEq,Clone,Decodable)]
pub struct ConfigValue {
    value: ConfigValueValue,
    path: Vec<Path>
}

impl ConfigValue {
    pub fn new() -> ConfigValue {
        ConfigValue { value: List(vec!()), path: vec!() }
    }

    pub fn get_value<'a>(&'a self) -> &'a ConfigValueValue {
        &self.value
    }

    fn from_toml(path: &Path, toml: toml::Value) -> CargoResult<ConfigValue> {
        let value = match toml {
            toml::String(val) => String(val),
            toml::Array(val) => {
                List(try!(result::collect(val.move_iter().map(|toml| {
                    match toml {
                        toml::String(val) => Ok(val),
                        _ => Err(internal("")),
                    }
                }))))
            }
            toml::Table(val) => {
                Table(try!(result::collect(val.move_iter().map(|(key, value)| {
                    let value = raw_try!(ConfigValue::from_toml(path, value));
                    Ok((key, value))
                }))))
            }
            _ => return Err(internal(""))
        };

        Ok(ConfigValue { value: value, path: vec![path.clone()] })
    }

    fn merge(&mut self, from: ConfigValue) -> CargoResult<()> {
        let ConfigValue { value, path } = from;
        match (&mut self.value, value) {
            (&String(ref mut old), String(ref mut new)) => {
                mem::swap(old, new);
                self.path = path;
            }
            (&List(ref mut old), List(ref mut new)) => {
                old.extend(mem::replace(new, Vec::new()).move_iter());
                self.path.extend(path.move_iter());
            }
            (&Table(ref mut old), Table(ref mut new)) => {
                let new = mem::replace(new, HashMap::new());
                for (key, value) in new.move_iter() {
                    let mut err = Ok(());
                    old.find_with_or_insert_with(key, value,
                                                 |_, old, new| err = old.merge(new),
                                                 |_, new| new);
                    try!(err);
                }
                self.path.extend(path.move_iter());
            }
            (expected, found) => {
                return Err(internal(format!("expected {}, but found {}",
                                            expected.desc(), found.desc())))
            }
        }

        Ok(())
    }

    pub fn string<'a>(&'a self) -> CargoResult<&'a str> {
        match self.value {
            Table(_) => Err(internal("expected a string, but found a table")),
            List(_) => Err(internal("expected a string, but found a list")),
            String(ref s) => Ok(s.as_slice()),
        }
    }

    pub fn table<'a>(&'a self) -> CargoResult<&'a HashMap<String, ConfigValue>> {
        match self.value {
            String(_) => Err(internal("expected a table, but found a string")),
            List(_) => Err(internal("expected a table, but found a list")),
            Table(ref table) => Ok(table),
        }
    }

    pub fn list<'a>(&'a self) -> CargoResult<&'a [String]> {
        match self.value {
            String(_) => Err(internal("expected a list, but found a string")),
            Table(_) => Err(internal("expected a list, but found a table")),
            List(ref list) => Ok(list.as_slice()),
        }
    }
}

impl ConfigValueValue {
    fn desc(&self) -> &'static str {
        match *self {
            Table(..) => "table",
            List(..) => "array",
            String(..) => "string",
        }
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for ConfigValue {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        s.emit_map(2, |s| {
            raw_try!(s.emit_map_elt_key(0, |s| "value".encode(s)));
            raw_try!(s.emit_map_elt_val(0, |s| self.value.encode(s)));
            Ok(())
        })
    }
}

impl fmt::Show for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let paths: Vec<String> = self.path.iter().map(|p| {
            p.display().to_string()
        }).collect();
        write!(f, "{} (from {})", self.value, paths)
    }
}

pub fn get_config(pwd: Path, key: &str) -> CargoResult<ConfigValue> {
    find_in_tree(&pwd, |file| extract_config(file, key)).map_err(|_|
        human(format!("`{}` not found in your configuration", key)))
}

pub fn all_configs(pwd: Path) -> CargoResult<HashMap<String, ConfigValue>> {
    let mut cfg = ConfigValue { value: Table(HashMap::new()), path: Vec::new() };

    try!(walk_tree(&pwd, |mut file| {
        let path = file.path().clone();
        let contents = try!(file.read_to_string());
        let file = path.filename_display().to_string();
        let table = try!(cargo_toml::parse(contents.as_slice(),
                                           file.as_slice()).chain_error(|| {
            internal(format!("could not parse Toml manifest; path={}",
                             path.display()))
        }));
        let value = try!(ConfigValue::from_toml(&path, toml::Table(table)));
        try!(cfg.merge(value));
        Ok(())
    }).map_err(|_| human("Couldn't load Cargo configuration")));


    match cfg.value {
        Table(map) => Ok(map),
        _ => unreachable!(),
    }
}

fn find_in_tree<T>(pwd: &Path,
                   walk: |io::fs::File| -> CargoResult<T>) -> CargoResult<T> {
    let mut current = pwd.clone();

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let file = try!(io::fs::File::open(&possible));

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
             walk: |io::fs::File| -> CargoResult<()>) -> CargoResult<()> {
    let mut current = pwd.clone();
    let mut err = false;

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let file = try!(io::fs::File::open(&possible));

            match walk(file) {
                Err(_) => err = false,
                _ => ()
            }
        }

        if err { return Err(internal("")); }
        if !current.pop() { break; }
    }

    Ok(())
}

fn extract_config(mut file: io::fs::File, key: &str) -> CargoResult<ConfigValue> {
    let contents = try!(file.read_to_string());
    let mut toml = try!(cargo_toml::parse(contents.as_slice(),
                                          file.path().filename_display()
                                              .to_string().as_slice()));
    let val = try!(toml.pop_equiv(&key).require(|| internal("")));

    ConfigValue::from_toml(file.path(), val)
}
