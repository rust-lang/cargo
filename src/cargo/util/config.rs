use std::{io,fmt};
use collections::HashMap;
use serialize::{Encodable,Encoder};
use toml;
use util::{other_error,CargoResult,Require};

#[deriving(Eq,TotalEq,Clone,Encodable,Decodable)]
pub enum Location {
    Project,
    Global
}

#[deriving(Eq,TotalEq,Clone,Decodable)]
pub enum ConfigValueValue {
    String(StrBuf),
    List(Vec<StrBuf>)
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

#[deriving(Eq,TotalEq,Clone,Decodable)]
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
        let paths: Vec<StrBuf> = self.path.iter().map(|p| format_strbuf!("{}", p.display())).collect();
        write!(f, "{} (from {})", self.value, paths)
    }
}

pub fn get_config(pwd: Path, key: &str) -> CargoResult<ConfigValue> {
    find_in_tree(&pwd, |file| extract_config(file, key))
        .map_err(|_| other_error("config key not found").with_detail(format!("key={}", key)))
}

pub fn all_configs(pwd: Path) -> CargoResult<HashMap<StrBuf, ConfigValue>> {
    let mut map = HashMap::new();

    try!(walk_tree(&pwd, |file| {
        extract_all_configs(file, &mut map)
    }));

    Ok(map)
}

#[allow(unused_variable)]
pub fn set_config(key: StrBuf, value: StrBuf, location: Location) -> CargoResult<()> {
    Ok(())
}

fn find_in_tree<T>(pwd: &Path, walk: |io::fs::File| -> CargoResult<T>) -> CargoResult<T> {
    let mut current = pwd.clone();

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let file = try!(io::fs::File::open(&possible).map_err(|_| other_error("could not open file")));
            match walk(file) {
                Ok(res) => return Ok(res),
                _ => ()
            }
        }

        if !current.pop() { break; }
    }

    Err(other_error(""))
}

fn walk_tree(pwd: &Path, walk: |io::fs::File| -> CargoResult<()>) -> CargoResult<()> {
    let mut current = pwd.clone();
    let mut err = false;

    loop {
        let possible = current.join(".cargo").join("config");
        if possible.exists() {
            let file = try!(io::fs::File::open(&possible).map_err(|_| other_error("could not open file")));
            match walk(file) {
                Err(_) => err = false,
                _ => ()
            }
        }

        if err { return Err(other_error("")); }
        if !current.pop() { break; }
    }

    Ok(())
}

fn extract_config(file: io::fs::File, key: &str) -> CargoResult<ConfigValue> {
    let path = file.path().clone();
    let mut buf = io::BufferedReader::new(file);
    let root = try!(toml::parse_from_buffer(&mut buf).map_err(|_| other_error("")));
    let val = try!(root.lookup(key).require(other_error("")));

    let v = match val {
        &toml::String(ref val) => String(val.clone()),
        &toml::Array(ref val) => List(val.iter().map(|s: &toml::Value| format_strbuf!("{}", s)).collect()),
        _ => return Err(other_error(""))
    };

    Ok(ConfigValue{ value: v, path: vec!(path) })
}

fn extract_all_configs(file: io::fs::File, map: &mut HashMap<StrBuf, ConfigValue>) -> CargoResult<()> {
    let path = file.path().clone();
    let mut buf = io::BufferedReader::new(file);
    let root = try!(toml::parse_from_buffer(&mut buf).map_err(|err|
        other_error("could not parse Toml manifest").with_detail(format!("path={}; err={}", path.display(), err.to_str()))));

    let table = try!(root.get_table()
        .require(other_error("could not parse Toml manifest").with_detail(format!("path={}", path.display()))));

    for (key, value) in table.iter() {
        match value {
            &toml::String(ref val) => { map.insert(key.clone(), ConfigValue { value: String(val.clone()), path: vec!(path.clone()) }); }
            &toml::Array(ref val) => {
                let config = map.find_or_insert_with(key.clone(), |_| {
                    ConfigValue { path: vec!(), value: List(vec!()) }
                });

                try!(merge_array(config, val.as_slice(), &path).map_err(|err|
                    other_error("missing").with_detail(format!("The `{}` key in your config {}", key, err))));
            },
            _ => ()
        }
    }

    Ok(())
}

fn merge_array(existing: &mut ConfigValue, val: &[toml::Value], path: &Path) -> CargoResult<()> {
    match existing.value {
        String(_) => return Err(other_error("should be an Array, but it was a String")),
        List(ref mut list) => {
            let new_list: Vec<CargoResult<StrBuf>> = val.iter().map(|s: &toml::Value| toml_string(s)).collect();
            if new_list.iter().any(|v| v.is_err()) {
                return Err(other_error("should be an Array of Strings, but was an Array of other values"));
            } else {
                let new_list: Vec<StrBuf> = new_list.move_iter().map(|v| v.unwrap()).collect();
                list.push_all(new_list.as_slice());
                existing.path.push(path.clone());
                Ok(())
            }
        }
    }
}

fn toml_string(val: &toml::Value) -> CargoResult<StrBuf> {
    match val {
        &toml::String(ref str) => Ok(str.clone()),
        _ => Err(other_error(""))
    }
}
