extern crate toml;

use super::super::{CargoResult,CargoError,ToCargoError};
use std::{io,fmt};

#[deriving(Eq,Clone,Encodable,Decodable)]
pub enum Location {
    Project,
    Global
}

#[deriving(Eq,Clone,Encodable,Decodable)]
pub struct ConfigValue {
    value: ~str,
    path: ~str
}

impl fmt::Show for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f.buf, "{} (from {})", self.value, self.path)
    }
}

pub fn get_config(pwd: Path, key: &str) -> CargoResult<ConfigValue> {
    walk_tree(&pwd, |file| extract_config(file, key)).to_cargo_error(format!("Config key not found: {}", key), 1)
}

pub fn set_config(key: ~str, value: ~str, location: Location) -> CargoResult<()> {
    Ok(())
}

fn walk_tree<T>(pwd: &Path, walk: |io::fs::File| -> CargoResult<T>) -> Option<T> {
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

fn extract_config(file: io::fs::File, key: &str) -> CargoResult<ConfigValue> {
    let path = try!(file.path().as_str().to_cargo_error(~"", 1)).to_owned();
    let mut buf = io::BufferedReader::new(file);
    let root = try!(toml::parse_from_buffer(&mut buf).to_cargo_error(~"", 1));
    let val = try!(try!(root.lookup(key).to_cargo_error(~"", 1)).get_str().to_cargo_error(~"", 1));
    Ok(ConfigValue{ value: val.to_owned(), path: path })
}
