#![crate_id="cargo-compile"]
#![allow(deprecated_owned_vector)]

extern crate serialize;
extern crate hammer;
extern crate cargo;

use serialize::{Decodable};
use hammer::{FlagDecoder,FlagConfig,FlagConfiguration,HammerError};
use std::io;
use io::BufReader;
use io::process::{Process,ProcessExit,ProcessOutput,InheritFd,ProcessConfig};
use cargo::{ToCargoError,CargoResult};

#[deriving(Decodable)]
struct Options {
    manifest_path: ~str
}

impl FlagConfig for Options {
    fn config(_: Option<Options>, c: FlagConfiguration) -> FlagConfiguration { c }
}

fn main() {
    match compile() {
        Err(io_error) => fail!("{}", io_error),
        Ok(_) => return
    }
}

fn compile() -> CargoResult<()> {
    let options = try!(flags::<Options>());
    let manifest_bytes = try!(read_manifest(options.manifest_path).to_cargo_error(~"Could not read manifest", 1));

    call_rustc(~BufReader::new(manifest_bytes.as_slice()))
}

fn flags<T: FlagConfig + Decodable<FlagDecoder, HammerError>>() -> CargoResult<T> {
    let mut decoder = FlagDecoder::new::<T>(std::os::args().tail());
    Decodable::decode(&mut decoder).to_cargo_error(|e: HammerError| e.message, 1)
}

fn read_manifest(manifest_path: &str) -> CargoResult<~[u8]> {
    Ok((try!(exec_with_output("cargo-read-manifest", [~"--manifest-path", manifest_path.to_owned()], None))).output)
}

fn call_rustc(mut manifest_data: ~Reader:) -> CargoResult<()> {
    let data: &mut Reader = manifest_data;
    try!(exec_tty("cargo-rustc", [], Some(data)));
    Ok(())
}

fn exec_with_output(program: &str, args: &[~str], input: Option<&mut Reader>) -> CargoResult<ProcessOutput> {
    Ok((try!(exec(program, args, input, |_| {}))).wait_with_output())
}

fn exec_tty(program: &str, args: &[~str], input: Option<&mut Reader>) -> CargoResult<ProcessExit> {
    Ok((try!(exec(program, args, input, |config| {
        config.stdout = InheritFd(1);
        config.stderr = InheritFd(2);
    }))).wait())
}

fn exec(program: &str, args: &[~str], input: Option<&mut Reader>, configurator: |&mut ProcessConfig|) -> CargoResult<Process> {
    let mut config = ProcessConfig::new();
    config.program = program;
    config.args = args;
    configurator(&mut config);

    println!("Executing {} {}", program, args);

    let mut process = try!(Process::configure(config).to_cargo_error(~"Could not configure process", 1));

    input.map(|mut reader| io::util::copy(&mut reader, process.stdin.get_mut_ref()));

    Ok(process)
}
