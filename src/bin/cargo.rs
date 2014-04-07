extern crate cargo;
extern crate toml;
extern crate hammer;
extern crate serialize;
extern crate collections;

use hammer::{FlagConfig,FlagConfiguration};
use std::{os,io};
use serialize::{Decodable,Encodable,json};
use cargo::{CargoResult,ToCargoError,NoFlags,execute_main_without_stdin,process_executed,handle_error};
use cargo::util::important_paths::find_project;
use cargo::util::config;

fn main() {
    execute();
}

#[deriving(Encodable)]
struct ProjectLocation {
    root: ~str
}

fn execute() {
    let (cmd, args) = match process(os::args()) {
        Ok((cmd, args)) => (cmd, args),
        Err(err) => return handle_error(err)
    };

    if cmd == ~"config" { execute_main_without_stdin(config) }
    else if cmd == ~"locate-project" { execute_main_without_stdin(locate_project) }
}

fn process(mut args: ~[~str]) -> CargoResult<(~str, ~[~str])> {
    args = args.tail().to_owned();
    let head = try!(args.head().to_cargo_error(~"No subcommand found", 1)).to_owned();
    let tail = args.tail().to_owned();

    Ok((head, tail))
}

#[deriving(Decodable)]
struct ConfigFlags {
    key: ~str,
    value: Option<~str>,
    human: bool
}

impl FlagConfig for ConfigFlags {
    fn config(_: Option<ConfigFlags>, c: FlagConfiguration) -> FlagConfiguration {
        c.short("human", 'h')
    }
}

#[deriving(Encodable)]
struct ConfigOut {
    values: collections::HashMap<~str, config::ConfigValue>
}

fn config(args: ConfigFlags) -> CargoResult<Option<ConfigOut>> {
    let value = try!(config::get_config(os::getcwd(), args.key.as_slice()));

    if args.human {
        println!("{}", value);
        Ok(None)
    } else {
        let mut map = collections::HashMap::new();
        map.insert(args.key.clone(), value);
        Ok(Some(ConfigOut { values: map }))
    }
}

fn locate_project(args: NoFlags) -> CargoResult<Option<ProjectLocation>> {
    let root = try!(find_project(os::getcwd(), ~"Cargo.toml"));
    let string = try!(root.as_str().to_cargo_error(format!("Your project path contains characters not representable in Unicode: {}", os::getcwd().display()), 1));
    Ok(Some(ProjectLocation { root: string.to_owned() }))
}
