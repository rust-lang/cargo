use std::cell::{RefCell, RefMut, Ref, Cell};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_map::{HashMap};
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::prelude::*;
use std::mem;
use std::path::{Path, PathBuf};

use rustc_serialize::{Encodable,Encoder};
use toml;
use core::{MultiShell, Package};
use util::{CargoResult, ChainError, Rustc, internal, human, paths};

use util::toml as cargo_toml;

use self::ConfigValue as CV;

pub struct Config {
    home_path: PathBuf,
    shell: RefCell<MultiShell>,
    rustc_info: Rustc,
    values: RefCell<HashMap<String, ConfigValue>>,
    values_loaded: Cell<bool>,
    cwd: PathBuf,
    rustc: PathBuf,
    rustdoc: PathBuf,
    target_dir: RefCell<Option<PathBuf>>,
}

impl Config {
    pub fn new(shell: MultiShell, cwd: PathBuf) -> CargoResult<Config> {
        let mut cfg = Config {
            home_path: try!(homedir(cwd.as_path()).chain_error(|| {
                human("Cargo couldn't find your home directory. \
                      This probably means that $HOME was not set.")
            })),
            shell: RefCell::new(shell),
            rustc_info: Rustc::blank(),
            cwd: cwd,
            values: RefCell::new(HashMap::new()),
            values_loaded: Cell::new(false),
            rustc: PathBuf::from("rustc"),
            rustdoc: PathBuf::from("rustdoc"),
            target_dir: RefCell::new(None),
        };

        try!(cfg.scrape_tool_config());
        try!(cfg.scrape_rustc_version());
        try!(cfg.scrape_target_dir_config());

        Ok(cfg)
    }

    pub fn home(&self) -> &Path { &self.home_path }

    pub fn git_db_path(&self) -> PathBuf {
        self.home_path.join("git").join("db")
    }

    pub fn git_checkout_path(&self) -> PathBuf {
        self.home_path.join("git").join("checkouts")
    }

    pub fn registry_index_path(&self) -> PathBuf {
        self.home_path.join("registry").join("index")
    }

    pub fn registry_cache_path(&self) -> PathBuf {
        self.home_path.join("registry").join("cache")
    }

    pub fn registry_source_path(&self) -> PathBuf {
        self.home_path.join("registry").join("src")
    }

    pub fn shell(&self) -> RefMut<MultiShell> {
        self.shell.borrow_mut()
    }

    pub fn rustc(&self) -> &Path { &self.rustc }

    pub fn rustdoc(&self) -> &Path { &self.rustdoc }

    pub fn rustc_info(&self) -> &Rustc { &self.rustc_info }

    pub fn values(&self) -> CargoResult<Ref<HashMap<String, ConfigValue>>> {
        if !self.values_loaded.get() {
            try!(self.load_values());
            self.values_loaded.set(true);
        }
        Ok(self.values.borrow())
    }

    pub fn cwd(&self) -> &Path { &self.cwd }

    pub fn target_dir(&self, pkg: &Package) -> PathBuf {
        self.target_dir.borrow().clone().unwrap_or_else(|| {
            pkg.root().join("target")
        })
    }

    pub fn set_target_dir(&self, path: &Path) {
        *self.target_dir.borrow_mut() = Some(path.to_owned());
    }

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
                CV::Integer(_, ref path) |
                CV::String(_, ref path) |
                CV::List(_, ref path) |
                CV::Boolean(_, ref path) => {
                    let idx = key.split('.').take(i)
                                 .fold(0, |n, s| n + s.len()) + i - 1;
                    let key_so_far = &key[..idx];
                    return Err(human(format!("expected table for configuration \
                                              key `{}`, but found {} in {}",
                                             key_so_far, val.desc(),
                                             path.display())));
                }
            }
        }
        Ok(Some(val.clone()))
    }

    pub fn get_string(&self, key: &str) -> CargoResult<Option<(String, PathBuf)>> {
        match try!(self.get(key)) {
            Some(CV::String(i, path)) => Ok(Some((i, path))),
            Some(val) => self.expected("string", key, val),
            None => Ok(None),
        }
    }

    pub fn get_path(&self, key: &str) -> CargoResult<Option<PathBuf>> {
        if let Some((specified_path, path_to_config)) = try!(self.get_string(&key)) {
            if specified_path.contains("/") || (cfg!(windows) && specified_path.contains("\\")) {
                // An absolute or a relative path
                let prefix_path = path_to_config.parent().unwrap().parent().unwrap();
                // Joining an absolute path to any path results in the given absolute path
                Ok(Some(prefix_path.join(specified_path)))
            } else {
                // A pathless name
                Ok(Some(PathBuf::from(specified_path)))
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_list(&self, key: &str) -> CargoResult<Option<(Vec<(String, PathBuf)>, PathBuf)>> {
        match try!(self.get(key)) {
            Some(CV::List(i, path)) => Ok(Some((i, path))),
            Some(val) => self.expected("list", key, val),
            None => Ok(None),
        }
    }

    pub fn get_table(&self, key: &str)
                    -> CargoResult<Option<(HashMap<String, CV>, PathBuf)>> {
        match try!(self.get(key)) {
            Some(CV::Table(i, path)) => Ok(Some((i, path))),
            Some(val) => self.expected("table", key, val),
            None => Ok(None),
        }
    }

    pub fn get_i64(&self, key: &str) -> CargoResult<Option<(i64, PathBuf)>> {
        match try!(self.get(key)) {
            Some(CV::Integer(i, path)) => Ok(Some((i, path))),
            Some(val) => self.expected("integer", key, val),
            None => Ok(None),
        }
    }

    pub fn expected<T>(&self, ty: &str, key: &str, val: CV) -> CargoResult<T> {
        val.expected(ty).map_err(|e| {
            human(format!("invalid configuration for key `{}`\n{}", key, e))
        })
    }

    fn load_values(&self) -> CargoResult<()> {
        let mut cfg = CV::Table(HashMap::new(), PathBuf::from("."));

        try!(walk_tree(&self.cwd, |mut file, path| {
            let mut contents = String::new();
            try!(file.read_to_string(&mut contents));
            let table = try!(cargo_toml::parse(&contents, &path).chain_error(|| {
                human(format!("could not parse TOML configuration in `{}`",
                              path.display()))
            }));
            let toml = toml::Value::Table(table);
            let value = try!(CV::from_toml(&path, toml).chain_error(|| {
                human(format!("failed to load TOML configuration from `{}`",
                              path.display()))
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

    fn scrape_tool_config(&mut self) -> CargoResult<()> {
        self.rustc = try!(self.get_tool("rustc"));
        self.rustdoc = try!(self.get_tool("rustdoc"));
        Ok(())
    }

    fn scrape_rustc_version(&mut self) -> CargoResult<()> {
        self.rustc_info = try!(Rustc::new(&self.rustc, self.cwd()));
        Ok(())
    }

    fn scrape_target_dir_config(&mut self) -> CargoResult<()> {
        if let Some((dir, dir2)) = try!(self.get_string("build.target-dir")) {
            let mut path = PathBuf::from(dir2);
            path.pop();
            path.pop();
            path.push(dir);
            *self.target_dir.borrow_mut() = Some(path);
        } else if let Some(dir) = env::var_os("CARGO_TARGET_DIR") {
            *self.target_dir.borrow_mut() = Some(self.cwd.join(dir));
        }
        Ok(())
    }

    fn get_tool(&self, tool: &str) -> CargoResult<PathBuf> {
        let var = format!("build.{}", tool);
        if let Some(tool_path) = try!(self.get_path(&var)) {
            return Ok(tool_path);
        }

        let var = tool.chars().flat_map(|c| c.to_uppercase()).collect::<String>();
        let tool = env::var_os(&var).unwrap_or_else(|| OsString::from(tool));
        Ok(PathBuf::from(tool))
    }
}

#[derive(Eq, PartialEq, Clone, RustcEncodable, RustcDecodable, Copy)]
pub enum Location {
    Project,
    Global
}

#[derive(Eq,PartialEq,Clone,RustcDecodable)]
pub enum ConfigValue {
    Integer(i64, PathBuf),
    String(String, PathBuf),
    List(Vec<(String, PathBuf)>, PathBuf),
    Table(HashMap<String, ConfigValue>, PathBuf),
    Boolean(bool, PathBuf),
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
                try!(write!(f, "["));
                for (i, &(ref s, ref path)) in list.iter().enumerate() {
                    if i > 0 { try!(write!(f, ", ")); }
                    try!(write!(f, "{} (from {})", s, path.display()));
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
                Ok(CV::List(try!(val.into_iter().map(|toml| {
                    match toml {
                        toml::Value::String(val) => Ok((val, path.to_path_buf())),
                        v => Err(human(format!("expected string but found {} \
                                                in list", v.type_str()))),
                    }
                }).collect::<CargoResult<_>>()), path.to_path_buf()))
            }
            toml::Value::Table(val) => {
                Ok(CV::Table(try!(val.into_iter().map(|(key, value)| {
                    let value = try!(CV::from_toml(path, value).chain_error(|| {
                        human(format!("failed to parse key `{}`", key))
                    }));
                    Ok((key, value))
                }).collect::<CargoResult<_>>()), path.to_path_buf()))
            }
            v => return Err(human(format!("found TOML configuration value of \
                                           unknown type `{}`", v.type_str())))
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
                            try!(entry.merge(value).chain_error(|| {
                                human(format!("failed to merge key `{}` between \
                                               files:\n  \
                                               file 1: {}\n  \
                                               file 2: {}",
                                              key,
                                              entry.definition_path().display(),
                                              path.display()))

                            }));
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

    pub fn i64(&self) -> CargoResult<(i64, &Path)> {
        match *self {
            CV::Integer(i, ref p) => Ok((i, p)),
            _ => self.expected("integer"),
        }
    }

    pub fn string(&self) -> CargoResult<(&str, &Path)> {
        match *self {
            CV::String(ref s, ref p) => Ok((s, p)),
            _ => self.expected("string"),
        }
    }

    pub fn table(&self) -> CargoResult<(&HashMap<String, ConfigValue>, &Path)> {
        match *self {
            CV::Table(ref table, ref p) => Ok((table, p)),
            _ => self.expected("table"),
        }
    }

    pub fn list(&self) -> CargoResult<&[(String, PathBuf)]> {
        match *self {
            CV::List(ref list, _) => Ok(list),
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

    fn expected<T>(&self, wanted: &str) -> CargoResult<T> {
        Err(internal(format!("expected a {}, but found a {} in {}",
                             wanted, self.desc(),
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

fn homedir(cwd: &Path) -> Option<PathBuf> {
    let cargo_home = env::var_os("CARGO_HOME").map(|home| {
        cwd.join(home)
    });
    let user_home = env::home_dir().map(|p| p.join(".cargo"));
    return cargo_home.or(user_home);
}

fn walk_tree<F>(pwd: &Path, mut walk: F) -> CargoResult<()>
    where F: FnMut(File, &Path) -> CargoResult<()>
{
    let mut current = pwd;

    loop {
        let possible = current.join(".cargo").join("config");
        if fs::metadata(&possible).is_ok() {
            let file = try!(File::open(&possible));

            try!(walk(file, &possible));
        }
        match current.parent() {
            Some(p) => current = p,
            None => break,
        }
    }

    // Once we're done, also be sure to walk the home directory even if it's not
    // in our history to be sure we pick up that standard location for
    // information.
    let home = try!(homedir(pwd).chain_error(|| {
        human("Cargo couldn't find your home directory. \
              This probably means that $HOME was not set.")
    }));
    if !pwd.starts_with(&home) {
        let config = home.join("config");
        if fs::metadata(&config).is_ok() {
            let file = try!(File::open(&config));
            try!(walk(file, &config));
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
    try!(fs::create_dir_all(file.parent().unwrap()));
    let contents = paths::read(&file).unwrap_or(String::new());
    let mut toml = try!(cargo_toml::parse(&contents, &file));
    toml.insert(key.to_string(), value.into_toml());

    let contents = toml::Value::Table(toml).to_string();
    try!(paths::write(&file, contents.as_bytes()));
    Ok(())
}
