extern crate collections;
extern crate toml;

use super::super::{CargoResult,ToCargoError,CargoError};
use std::{io,fmt};

#[deriving(Eq,TotalEq,Clone,Encodable,Decodable)]
pub enum Location {
    Project,
    Global
}

#[deriving(Eq,TotalEq,Clone,Encodable,Decodable,Show)]
enum ConfigValueValue {
    String(~str),
    List(~[~str])
}

#[deriving(Eq,TotalEq,Clone,Encodable,Decodable)]
pub struct ConfigValue {
    value: ConfigValueValue,
    path: ~str
}

impl fmt::Show for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f.buf, "{} (from {})", self.value, self.path)
    }
}

pub fn get_config(pwd: Path, key: &str) -> CargoResult<ConfigValue> {
    find_in_tree(&pwd, |file| extract_config(file, key)).to_cargo_error(format!("Config key not found: {}", key), 1)
}

pub fn all_configs(pwd: Path) -> CargoResult<collections::HashMap<~str, ConfigValue>> {
    let mut map = collections::HashMap::new();

    walk_tree(&pwd, |file| {
        let _ = extract_all_configs(file).map(|configs| {
            for (key, value) in configs.move_iter() {
                map.find_or_insert(key, value);
            }
        });
    });

    Ok(map)
}

#[allow(unused_variable)]
pub fn set_config(key: ~str, value: ~str, location: Location) -> CargoResult<()> {
    Ok(())
}

fn find_in_tree<T>(pwd: &Path, walk: |io::fs::File| -> CargoResult<T>) -> Option<T> {
    let mut current = pwd.clone();

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let res = io::fs::File::open(&possible).map(|file| walk(file));

            match res {
                Ok(Ok(res)) => return Some(res),
                _ => ()
            }
        }

        if !current.pop() { break; }
    }

    None
}

fn walk_tree(pwd: &Path, walk: |io::fs::File| -> ()) {
    let mut current = pwd.clone();

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let _ = io::fs::File::open(&possible).map(|file| walk(file));
        }

        if !current.pop() { break; }
    }
}

fn extract_config(file: io::fs::File, key: &str) -> CargoResult<ConfigValue> {
    let path = try!(file.path().as_str().to_cargo_error(~"", 1)).to_owned();
    let mut buf = io::BufferedReader::new(file);
    let root = try!(toml::parse_from_buffer(&mut buf).to_cargo_error(~"", 1));
    let val = try!(root.lookup(key).to_cargo_error(~"", 1));

    let v = match val {
        &toml::String(ref val) => String(val.to_owned()),
        &toml::Array(ref val) => List(val.iter().map(|s: &toml::Value| s.to_str()).collect()),
        _ => return Err(CargoError::new(~"", 1))
    };

    Ok(ConfigValue{ value: v, path: path })
}

fn extract_all_configs(file: io::fs::File) -> CargoResult<collections::HashMap<~str, ConfigValue>> {
    let mut map = collections::HashMap::new();

    let path = try!(file.path().as_str().to_cargo_error(~"", 1)).to_owned();
    let mut buf = io::BufferedReader::new(file);
    let root = try!(toml::parse_from_buffer(&mut buf).to_cargo_error(~"", 1));
    let table = try!(root.get_table().to_cargo_error(~"", 1));

    for (key, value) in table.iter() {
        match value {
            &toml::String(ref val) => { map.insert(key.to_owned(), ConfigValue { value: String(val.to_owned()), path: path.clone() }); }
            &toml::Array(ref val) => { map.insert(key.to_owned(), ConfigValue { value: List(val.iter().map(|s: &toml::Value| s.to_str()).collect()), path: path.clone() }); }
            _ => ()
        }
    }

    Ok(map)
}
