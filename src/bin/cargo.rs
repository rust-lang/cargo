extern crate cargo;
extern crate toml;
extern crate hammer;
extern crate serialize;
extern crate collections;

use hammer::{FlagConfig,FlagConfiguration};
use std::os;
use serialize::Encodable;
use cargo::{CargoResult,ToCargoError,NoFlags,execute_main_without_stdin,handle_error};
use cargo::util::important_paths::find_project;
use cargo::util::config;

fn main() {
    execute();
}

#[deriving(Encodable)]
struct ProjectLocation {
    root: ~str
}

/**
  The top-level `cargo` command handles configuration and project location
  because they are fundamental (and intertwined). Other commands can rely
  on this top-level information.
*/
fn execute() {
    let (cmd, _) = match process(os::args()) {
        Ok((cmd, args)) => (cmd, args),
        Err(err) => return handle_error(err)
    };

    if cmd == "config-for-key".to_owned() { execute_main_without_stdin(config_for_key) }
    else if cmd == "config-list".to_owned() { execute_main_without_stdin(config_list) }
    else if cmd == "locate-project".to_owned() { execute_main_without_stdin(locate_project) }
}

fn process(mut args: ~[~str]) -> CargoResult<(~str, ~[~str])> {
    args = args.tail().to_owned();
    let head = try!(args.head().to_cargo_error("No subcommand found".to_owned(), 1)).to_owned();
    let tail = args.tail().to_owned();

    Ok((head, tail))
}

#[deriving(Encodable)]
struct ConfigOut {
    values: collections::HashMap<~str, config::ConfigValue>
}

#[deriving(Decodable)]
struct ConfigForKeyFlags {
    key: ~str,
    human: bool
}

impl FlagConfig for ConfigForKeyFlags {
    fn config(_: Option<ConfigForKeyFlags>, config: FlagConfiguration) -> FlagConfiguration {
        config.short("human", 'h')
    }
}

fn config_for_key(args: ConfigForKeyFlags) -> CargoResult<Option<ConfigOut>> {
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

#[deriving(Decodable)]
struct ConfigListFlags {
    human: bool
}

impl FlagConfig for ConfigListFlags {
    fn config(_: Option<ConfigListFlags>, config: FlagConfiguration) -> FlagConfiguration {
        config.short("human", 'h')
    }
}

fn config_list(args: ConfigListFlags) -> CargoResult<Option<ConfigOut>> {
    let configs = try!(config::all_configs(os::getcwd()));

    if args.human {
        for (key, value) in configs.iter() {
            println!("{} = {}", key, value);
        }
        Ok(None)
    } else {
        Ok(Some(ConfigOut { values: configs }))
    }
}

fn locate_project(_: NoFlags) -> CargoResult<Option<ProjectLocation>> {
    let root = try!(find_project(os::getcwd(), "Cargo.toml".to_owned()));
    let string = try!(root.as_str().to_cargo_error(format!("Your project path contains characters not representable in Unicode: {}", os::getcwd().display()), 1));
    Ok(Some(ProjectLocation { root: string.to_owned() }))
}
