use std::{io,fmt,os};
use std::collections::HashMap;
use serialize::{Encodable,Encoder};
use toml;
use util::{CargoResult, CargoError, ChainError, Require, internal, human};

pub struct Config {
    home_path: Path
}

impl Config {
    pub fn new() -> CargoResult<Config> {
        Ok(Config {
            home_path: cargo_try!(os::homedir().require(|| {
                human("Couldn't find the home directory")
            }))
        })
    }

    pub fn git_db_path(&self) -> Path {
        self.home_path.join(".cargo").join("git").join("db")
    }

    pub fn git_checkout_path(&self) -> Path {
        self.home_path.join(".cargo").join("git").join("checkouts")
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
                try!(string.encode(s));
            },
            &List(ref list) => {
                try!(list.encode(s));
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
            try!(s.emit_map_elt_key(0, |s| "value".encode(s)));
            try!(s.emit_map_elt_val(0, |s| self.value.encode(s)));
            Ok(())
        })
    }
}

impl fmt::Show for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let paths: Vec<String> = self.path.iter().map(|p| {
            p.display().to_str()
        }).collect();
        write!(f, "{} (from {})", self.value, paths)
    }
}

pub fn get_config(pwd: Path, key: &str) -> CargoResult<ConfigValue> {
    find_in_tree(&pwd, |file| extract_config(file, key)).map_err(|_|
        internal(format!("config key not found; key={}", key)))
}

pub fn all_configs(pwd: Path) -> CargoResult<HashMap<String, ConfigValue>> {
    let mut map = HashMap::new();

    cargo_try!(walk_tree(&pwd, |file| {
        extract_all_configs(file, &mut map)
    }));

    Ok(map)
}

fn find_in_tree<T>(pwd: &Path,
                   walk: |io::fs::File| -> CargoResult<T>) -> CargoResult<T> {
    let mut current = pwd.clone();

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let file = cargo_try!(io::fs::File::open(&possible).chain_error(|| {
                internal("could not open file")
            }));
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
            let file = cargo_try!(io::fs::File::open(&possible).chain_error(|| {
                internal("could not open file")
            }));
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

fn extract_config(file: io::fs::File, key: &str) -> CargoResult<ConfigValue> {
    let path = file.path().clone();
    let mut buf = io::BufferedReader::new(file);
    let root = cargo_try!(toml::parse_from_buffer(&mut buf));
    let val = cargo_try!(root.lookup(key).require(|| internal("")));

    let v = match *val {
        toml::String(ref val) => String(val.clone()),
        toml::Array(ref val) => {
            List(val.iter().map(|s: &toml::Value| s.to_str()).collect())
        }
        _ => return Err(internal(""))
    };

    Ok(ConfigValue{ value: v, path: vec!(path) })
}

fn extract_all_configs(file: io::fs::File,
                       map: &mut HashMap<String, ConfigValue>) -> CargoResult<()> {
    let path = file.path().clone();
    let mut buf = io::BufferedReader::new(file);
    let root = cargo_try!(toml::parse_from_buffer(&mut buf).chain_error(|| {
        internal(format!("could not parse Toml manifest; path={}",
                         path.display()))
    }));

    let table = cargo_try!(root.get_table().require(|| {
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

                cargo_try!(merge_array(config, val.as_slice(),
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
            let new_list: Vec<CargoResult<String>> =
                val.iter().map(toml_string).collect();
            if new_list.iter().any(|v| v.is_err()) {
                return Err(internal("should be an Array of Strings, but \
                                     was an Array of other values"));
            } else {
                let new_list: Vec<String> =
                    new_list.move_iter().map(|v| v.unwrap()).collect();
                list.push_all(new_list.as_slice());
                existing.path.push(path.clone());
                Ok(())
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
