#[crate_id="cargo-compile"];

extern crate serialize;
extern crate hammer;
// extern crate cargo;

use serialize::{Decodable};
use hammer::{FlagDecoder,FlagConfig,FlagConfiguration};
use std::io;
use io::{IoResult,IoError,OtherIoError,BufReader};
use io::process::{Process,ProcessExit,ProcessOutput,InheritFd,ProcessConfig};

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

fn compile() -> IoResult<()> {
    let options = try!(flags::<Options>());
    let manifest_bytes = try!(read_manifest(options.manifest_path));

    call_rustc(~BufReader::new(manifest_bytes.as_slice()))
}

fn flags<T: FlagConfig + Decodable<FlagDecoder>>() -> IoResult<T> {
    let mut decoder = FlagDecoder::new::<T>(std::os::args().tail());
    let flags: T = Decodable::decode(&mut decoder);

    if decoder.error.is_some() {
        Err(IoError{ kind: OtherIoError, desc: "could not decode flags", detail: Some(decoder.error.unwrap()) })
    } else {
        Ok(flags)
    }
}

fn read_manifest(manifest_path: &str) -> IoResult<~[u8]> {
    Ok((try!(exec_with_output("cargo-read-manifest", [~"--manifest-path", manifest_path.to_owned()], None))).output)
}

fn call_rustc(mut manifest_data: ~Reader:) -> IoResult<()> {
    let data: &mut Reader = manifest_data;
    try!(exec_tty("cargo-rustc", [], Some(data)));
    Ok(())
}

fn exec_with_output(program: &str, args: &[~str], input: Option<&mut Reader>) -> IoResult<ProcessOutput> {
    Ok((try!(exec(program, args, input, |_| {}))).wait_with_output())
}

fn exec_tty(program: &str, args: &[~str], input: Option<&mut Reader>) -> IoResult<ProcessExit> {
    Ok((try!(exec(program, args, input, |config| {
        config.stdout = InheritFd(1);
        config.stderr = InheritFd(2);
    }))).wait())
}

fn exec(program: &str, args: &[~str], input: Option<&mut Reader>, configurator: |&mut ProcessConfig|) -> IoResult<Process> {
    let mut config = ProcessConfig::new();
    config.program = program;
    config.args = args;
    configurator(&mut config);

    println!("Executing {} {}", program, args);

    let mut process = try!(Process::configure(config));

    input.map(|mut reader| io::util::copy(&mut reader, process.stdin.get_mut_ref()));

    Ok(process)
}
