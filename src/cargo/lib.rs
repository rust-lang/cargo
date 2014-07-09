#![crate_name="cargo"]
#![crate_type="rlib"]

#![feature(macro_rules, phase)]

extern crate debug;
extern crate term;
extern crate url;
extern crate serialize;
extern crate semver;
extern crate toml;

#[phase(plugin, link)]
extern crate hammer;

#[phase(plugin, link)]
extern crate log;

#[cfg(test)]
extern crate hamcrest;

use serialize::{Decoder, Encoder, Decodable, Encodable, json};
use std::io;
use std::io::{stdout, stderr};
use std::io::stdio::{stdout_raw, stderr_raw};
use hammer::{Flags, decode_args, usage};

use core::{Shell, MultiShell, ShellConfig};
use term::color::{BLACK};

pub use util::{CargoError, CliError, CliResult, human};

macro_rules! some(
    ($e:expr) => (
        match $e {
            Some(e) => e,
            None => return None
        }
    )
)

// Added so that the try! macro below can refer to cargo::util, while
// other external importers of this macro can use it as well.
//
// "Hygiene strikes again" - @acrichton
mod cargo {
    pub use super::util;
}

#[macro_export]
macro_rules! try (
    ($expr:expr) => ({
        use cargo::util::FromError;
        match $expr.map_err(FromError::from_error) {
            Ok(val) => val,
            Err(err) => return Err(err)
        }
    })
)

macro_rules! raw_try (
    ($expr:expr) => ({
        match $expr {
            Ok(val) => val,
            Err(err) => return Err(err)
        }
    })
)

pub mod core;
pub mod ops;
pub mod sources;
pub mod util;

trait RepresentsJSON : Decodable<json::Decoder, json::DecoderError> {}
impl<T: Decodable<json::Decoder, json::DecoderError>> RepresentsJSON for T {}

#[deriving(Decodable)]
pub struct NoFlags;

hammer_config!(NoFlags)

#[deriving(Show, Decodable)]
pub struct GlobalFlags {
    verbose: bool,
    help: bool,
    rest: Vec<String>
}

hammer_config!(GlobalFlags |c| {
    c.short("verbose", 'v').short("help", 'h')
})

pub fn execute_main<'a,
                    T: Flags,
                    U: RepresentsJSON,
                    V: Encodable<json::Encoder<'a>, io::IoError>>(
                        exec: fn(T, U, &mut MultiShell) -> CliResult<Option<V>>)
{
    fn call<'a,
            T: Flags,
            U: RepresentsJSON,
            V: Encodable<json::Encoder<'a>, io::IoError>>(
                exec: fn(T, U, &mut MultiShell) -> CliResult<Option<V>>,
                shell: &mut MultiShell,
                args: &[String])
        -> CliResult<Option<V>>
    {
        let flags = try!(flags_from_args::<T>(args));
        let json = try!(json_from_stdin::<U>());

        exec(flags, json, shell)
    }

    process::<T, V>(|rest, shell| call(exec, shell, rest));
}

pub fn execute_main_without_stdin<'a,
                                  T: Flags,
                                  V: Encodable<json::Encoder<'a>, io::IoError>>(
                                      exec: fn(T, &mut MultiShell) -> CliResult<Option<V>>)
{
    fn call<'a,
            T: Flags,
            V: Encodable<json::Encoder<'a>, io::IoError>>(
                exec: fn(T, &mut MultiShell) -> CliResult<Option<V>>,
                shell: &mut MultiShell,
                args: &[String])
        -> CliResult<Option<V>>
    {
        let flags = try!(flags_from_args::<T>(args));
        exec(flags, shell)
    }

    process::<T, V>(|rest, shell| call(exec, shell, rest));
}

fn process<'a,
           T: Flags,
           V: Encodable<json::Encoder<'a>, io::IoError>>(
               callback: |&[String], &mut MultiShell| -> CliResult<Option<V>>) {


    match global_flags() {
        Err(e) => handle_error(e, &mut shell(false)),
        Ok(val) => {
            let mut shell = shell(val.verbose);

            if val.help {
                let (desc, options) = usage::<T>(true);

                desc.map(|d| println!("{}\n", d));

                println!("Options:\n");

                print!("{}", options);

                let (_, options) = usage::<GlobalFlags>(false);
                print!("{}", options);
            } else {
                process_executed(callback(val.rest.as_slice(), &mut shell), &mut shell)
            }
        }
    }
}

pub fn process_executed<'a,
                        T: Encodable<json::Encoder<'a>, io::IoError>>(
                            result: CliResult<Option<T>>,
                            shell: &mut MultiShell)
{
    match result {
        Err(e) => handle_error(e, shell),
        Ok(encodable) => {
            encodable.map(|encodable| {
                let encoded = json::encode(&encodable);
                println!("{}", encoded);
            });
        }
    }
}

pub fn shell(verbose: bool) -> MultiShell {
    let tty = stderr_raw().isatty();
    let stderr = box stderr() as Box<Writer>;

    let config = ShellConfig { color: true, verbose: verbose, tty: tty };
    let err = Shell::create(stderr, config);

    let tty = stdout_raw().isatty();
    let stdout = box stdout() as Box<Writer>;

    let config = ShellConfig { color: true, verbose: verbose, tty: tty };
    let out = Shell::create(stdout, config);

    MultiShell::new(out, err, verbose)
}

pub fn handle_error(err: CliError, shell: &mut MultiShell) {
    log!(4, "handle_error; err={}", err);

    let CliError { error, exit_code, unknown, .. } = err;

    if unknown {
        let _ = shell.error("An unknown error occurred");
    } else {
        let _ = shell.error(error.to_string());
    }

    if error.cause().is_some() {
        let _ = shell.concise(|shell| {
            shell.err().say("\nTo learn more, run the command again with --verbose.", BLACK)
        });
    }

    let _ = shell.verbose(|shell| {
        let _ = handle_cause(error, shell);
        Ok(())
      });

    std::os::set_exit_status(exit_code as int);
}

fn handle_cause(err: &CargoError, shell: &mut MultiShell) {
    let _ = shell.err().say("\nCaused by:", BLACK);
    let _ = shell.err().say(format!("  {}", err.description()), BLACK);

    err.cause().map(|e| handle_cause(e, shell));
}

fn args() -> Vec<String> {
    std::os::args()
}

fn flags_from_args<T: Flags>(args: &[String]) -> CliResult<T> {
    decode_args(args).map_err(|e| {
        CliError::new(e.message, 1)
    })
}

fn global_flags() -> CliResult<GlobalFlags> {
    decode_args(args().tail()).map_err(|e| {
        CliError::new(e.message, 1)
    })
}

fn json_from_stdin<T: RepresentsJSON>() -> CliResult<T> {
    let mut reader = io::stdin();
    let input = try!(reader.read_to_string().map_err(|_| {
        CliError::new("Standard in did not exist or was not UTF-8", 1)
    }));

    let json = try!(json::from_str(input.as_slice()).map_err(|_| {
        CliError::new("Could not parse standard in as JSON", 1)
    }));
    let mut decoder = json::Decoder::new(json);

    Decodable::decode(&mut decoder).map_err(|_| {
        CliError::new("Could not process standard in as input", 1)
    })
}
