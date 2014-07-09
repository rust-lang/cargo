use std::{io,fmt,os, result};
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
}

impl<'a> Config<'a> {
    pub fn new<'a>(shell: &'a mut MultiShell,
                   update_remotes: bool,
                   jobs: Option<uint>) -> CargoResult<Config<'a>> {
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

    pub fn jobs(&mut self) -> uint {
        self.jobs
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
    List(Vec<String>)
}

impl fmt::Show for ConfigValueValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &String(ref string) => write!(f, "{}", string),
            &List(ref list) => write!(f, "{}", list)
        }
    }
}

impl<E, S: Encoder<E>> Encodable<S, E> for ConfigValueValue {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        match self {
            &String(ref string) => {
                raw_try!(string.encode(s));
            },
            &List(ref list) => {
                raw_try!(list.encode(s));
            }
        }

        Ok(())
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
    let mut map = HashMap::new();

    try!(walk_tree(&pwd, |file| extract_all_configs(file, &mut map)).map_err(|_|
        human("Couldn't load Cargo configuration")));


    Ok(map)
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
    let toml = try!(cargo_toml::parse(contents.as_slice(),
                                      file.path().filename_display()
                                          .to_string().as_slice()));
    let val = try!(toml.find_equiv(&key).require(|| internal("")));

    let v = match *val {
        toml::String(ref val) => String(val.clone()),
        toml::Array(ref val) => {
            List(val.iter().map(|s: &toml::Value| s.to_string()).collect())
        }
        _ => return Err(internal(""))
    };

    Ok(ConfigValue{ value: v, path: vec![file.path().clone()] })
}

fn extract_all_configs(mut file: io::fs::File,
                       map: &mut HashMap<String, ConfigValue>) -> CargoResult<()> {
    let path = file.path().clone();
    let contents = try!(file.read_to_string());
    let file = path.filename_display().to_string();
    let table = try!(cargo_toml::parse(contents.as_slice(),
                                       file.as_slice()).chain_error(|| {
        internal(format!("could not parse Toml manifest; path={}",
                         path.display()))
    }));

    for (key, value) in table.iter() {
        match value {
            &toml::String(ref val) => {
                map.insert(key.clone(), ConfigValue {
                    value: String(val.clone()),
                    path: vec!(path.clone())
                });
            }
            &toml::Array(ref val) => {
                let config = map.find_or_insert_with(key.clone(), |_| {
                    ConfigValue { path: vec!(), value: List(vec!()) }
                });

                try!(merge_array(config, val.as_slice(),
                                       &path).chain_error(|| {
                    internal(format!("The `{}` key in your config", key))
                }));
            },
            _ => ()
        }
    }

    Ok(())
}

fn merge_array(existing: &mut ConfigValue, val: &[toml::Value],
               path: &Path) -> CargoResult<()> {
    match existing.value {
        String(_) => Err(internal("should be an Array, but it was a String")),
        List(ref mut list) => {
            let r: CargoResult<Vec<String>> = result::collect(val.iter().map(toml_string));
            match r {
                Err(_) => Err(internal("should be an Array of Strings, but \
                                        was an Array of other values")),
                Ok(new_list) => {
                    list.push_all(new_list.as_slice());
                    existing.path.push(path.clone());
                    Ok(())
                }
            }
        }
    }
}

fn toml_string(val: &toml::Value) -> CargoResult<String> {
    match val {
        &toml::String(ref str) => Ok(str.clone()),
        _ => Err(internal(""))
    }
}
