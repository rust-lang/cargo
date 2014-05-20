extern crate cargo;
extern crate toml;
extern crate hammer;
extern crate serialize;
extern crate collections;

use hammer::{FlagConfig,FlagConfiguration};
use std::os;
use serialize::Encodable;
use cargo::{NoFlags,execute_main_without_stdin,handle_error};
use cargo::core::errors::{CLIError,CLIResult,ToResult};
use cargo::util::important_paths::find_project;
use cargo::util::config;

fn main() {
    execute();
}

#[deriving(Encodable)]
struct ProjectLocation {
    root: StrBuf
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

    if cmd == "config-for-key".to_strbuf() { execute_main_without_stdin(config_for_key) }
    else if cmd == "config-list".to_strbuf() { execute_main_without_stdin(config_list) }
    else if cmd == "locate-project".to_strbuf() { execute_main_without_stdin(locate_project) }
}

fn process(args: Vec<~str>) -> CLIResult<(StrBuf, Vec<StrBuf>)> {
    let args: Vec<StrBuf> = args.tail().iter().map(|a| a.to_strbuf()).collect();
    let head = try!(args.iter().nth(0).to_result(|_| CLIError::new("No subcommand found", None::<&str>, 1))).to_owned();
    let tail = Vec::from_slice(args.tail());

    Ok((head.to_strbuf(), tail))
}

#[deriving(Encodable)]
struct ConfigOut {
    values: collections::HashMap<StrBuf, config::ConfigValue>
}

#[deriving(Decodable)]
struct ConfigForKeyFlags {
    key: StrBuf,
    human: bool
}

impl FlagConfig for ConfigForKeyFlags {
    fn config(_: Option<ConfigForKeyFlags>, config: FlagConfiguration) -> FlagConfiguration {
        config.short("human", 'h')
    }
}

fn config_for_key(args: ConfigForKeyFlags) -> CLIResult<Option<ConfigOut>> {
    let value = try!(config::get_config(os::getcwd(), args.key.as_slice()).to_result(|err|
        CLIError::new("Couldn't load configuration", Some(err), 1)));

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

fn config_list(args: ConfigListFlags) -> CLIResult<Option<ConfigOut>> {
    let configs = try!(config::all_configs(os::getcwd()).to_result(|err|
        CLIError::new("Couldn't load conifguration", Some(err), 1)));

    if args.human {
        for (key, value) in configs.iter() {
            println!("{} = {}", key, value);
        }
        Ok(None)
    } else {
        Ok(Some(ConfigOut { values: configs }))
    }
}

fn locate_project(_: NoFlags) -> CLIResult<Option<ProjectLocation>> {
    let root = try!(find_project(os::getcwd(), "Cargo.toml").to_result(|err|
        CLIError::new(err.to_str(), None::<&str>, 1)));

    let string = try!(root.as_str().to_result(|_|
        CLIError::new(format!("Your project path contains characters not representable in Unicode: {}", os::getcwd().display()), None::<&str>, 1)));

    Ok(Some(ProjectLocation { root: string.to_strbuf() }))
}
