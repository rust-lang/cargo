#![feature(phase)]

extern crate cargo;
extern crate toml = "github.com/mneumann/rust-toml#toml";
extern crate hammer;
extern crate serialize;
#[phase(plugin, link)]
extern crate log;

use hammer::{FlagConfig,FlagConfiguration};
use std::os;
use std::io::process::{Command,InheritFd,ExitStatus,ExitSignal};
use serialize::Encodable;
use cargo::{GlobalFlags, NoFlags, execute_main_without_stdin, handle_error};
use cargo::util::important_paths::find_project;
use cargo::util::{CliError, CliResult, Require, config, human};

fn main() {
    execute();
}

#[deriving(Encodable)]
struct ProjectLocation {
    root: String
}

/**
  The top-level `cargo` command handles configuration and project location
  because they are fundamental (and intertwined). Other commands can rely
  on this top-level information.
*/
fn execute() {
    debug!("executing; cmd=cargo; args={}", os::args());

    let (cmd, args) = match process(os::args()) {
        Ok((cmd, args)) => (cmd, args),
        Err(err) => return handle_error(err, false)
    };

    match cmd.as_slice() {
        "config-for-key" => {
            log!(4, "cmd == config-for-key");
            execute_main_without_stdin(config_for_key)
        },
        "config-list" => {
            log!(4, "cmd == config-list");
            execute_main_without_stdin(config_list)
        },
        "locate-project" => {
            log!(4, "cmd == locate-project");
            execute_main_without_stdin(locate_project)
        },
        "--help" | "-h" | "help" | "-?" => {
            println!("Commands:");
            println!("  compile          # compile the current project\n");

            let (_, options) = hammer::usage::<GlobalFlags>(false);
            println!("Options (for all commands):\n\n{}", options);
        },
        _ => {
            let command = Command::new(format!("cargo-{}", cmd))
                .args(args.as_slice())
                .stdin(InheritFd(0))
                .stdout(InheritFd(1))
                .stderr(InheritFd(2))
                .status();

            match command {
                Ok(ExitStatus(0)) => (),
                Ok(ExitStatus(i)) | Ok(ExitSignal(i)) => {
                    handle_error(CliError::new("", i as uint), false)
                }
                Err(_) => handle_error(CliError::new("No such subcommand", 127), false)
            }
        }
    }
}

fn process(args: Vec<String>) -> CliResult<(String, Vec<String>)> {
    let args: Vec<String> = Vec::from_slice(args.tail());
    let head = try!(args.iter().nth(0).require(|| {
        human("No subcommand found")
    }).map_err(|err| CliError::from_boxed(err, 1))).to_str();
    let tail = Vec::from_slice(args.tail());

    Ok((head, tail))
}

#[deriving(Encodable)]
struct ConfigOut {
    values: std::collections::HashMap<String, config::ConfigValue>
}

#[deriving(Decodable)]
struct ConfigForKeyFlags {
    key: String,
    human: bool
}

impl FlagConfig for ConfigForKeyFlags {
    fn config(_: Option<ConfigForKeyFlags>,
              config: FlagConfiguration) -> FlagConfiguration {
        config.short("human", 'h')
    }
}

fn config_for_key(args: ConfigForKeyFlags) -> CliResult<Option<ConfigOut>> {
    let value = try!(config::get_config(os::getcwd(),
                                        args.key.as_slice()).map_err(|_| {
        CliError::new("Couldn't load configuration",  1)
    }));

    if args.human {
        println!("{}", value);
        Ok(None)
    } else {
        let mut map = std::collections::HashMap::new();
        map.insert(args.key.clone(), value);
        Ok(Some(ConfigOut { values: map }))
    }
}

#[deriving(Decodable)]
struct ConfigListFlags {
    human: bool
}

impl FlagConfig for ConfigListFlags {
    fn config(_: Option<ConfigListFlags>,
              config: FlagConfiguration) -> FlagConfiguration {
        config.short("human", 'h')
    }
}

fn config_list(args: ConfigListFlags) -> CliResult<Option<ConfigOut>> {
    let configs = try!(config::all_configs(os::getcwd()).map_err(|_|
        CliError::new("Couldn't load configuration", 1)));

    if args.human {
        for (key, value) in configs.iter() {
            println!("{} = {}", key, value);
        }
        Ok(None)
    } else {
        Ok(Some(ConfigOut { values: configs }))
    }
}

fn locate_project(_: NoFlags) -> CliResult<Option<ProjectLocation>> {
    let root = try!(find_project(os::getcwd(), "Cargo.toml").map_err(|e| {
        CliError::from_boxed(e, 1)
    }));

    let string = try!(root.as_str()
                      .require(|| human("Your project path contains characters \
                                         not representable in Unicode"))
                      .map_err(|e| CliError::from_boxed(e, 1)));

    Ok(Some(ProjectLocation { root: string.to_str() }))
}
