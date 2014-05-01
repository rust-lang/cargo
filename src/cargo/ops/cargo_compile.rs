/**
 * Cargo compile currently does the following steps:
 *
 * All configurations are already injected as environment variables via the main cargo command
 *
 * 1. Read the manifest
 * 2. Shell out to `cargo-resolve` with a list of dependencies and sources as stdin
 *    a. Shell out to `--do update` and `--do list` for each source
 *    b. Resolve dependencies and return a list of name/version/source
 * 3. Shell out to `--do download` for each source
 * 4. Shell out to `--do get` for each source, and build up the list of paths to pass to rustc -L
 * 5. Call `cargo-rustc` with the results of the resolver zipped together with the results of the `get`
 *    a. Topologically sort the dependencies
 *    b. Compile each dependency in order, passing in the -L's pointing at each previously compiled dependency
 */

use std;
use std::vec::Vec;
use serialize::{Decodable};
use hammer::{FlagDecoder,FlagConfig,FlagConfiguration,HammerError};
use std::io;
use std::io::BufReader;
use std::io::process::{Process,ProcessExit,ProcessOutput,InheritFd,ProcessConfig};
use std::os;
use util::config;
use util::config::{all_configs,ConfigValue};
use cargo_read_manifest = ops::cargo_read_manifest::read_manifest;
use core::Package;
use {CargoError,ToCargoError,CargoResult};

#[deriving(Decodable)]
struct Options {
    manifest_path: ~str
}

impl FlagConfig for Options {
    fn config(_: Option<Options>, c: FlagConfiguration) -> FlagConfiguration { c }
}

pub fn compile() -> CargoResult<()> {
    let options = try!(flags::<Options>());
    let manifest_bytes = try!(read_manifest(options.manifest_path));

    let configs = try!(all_configs(os::getcwd()));
    let config_paths = configs.find(&~"paths").map(|v| v.clone()).unwrap_or_else(|| ConfigValue::new());

    let paths = match config_paths.get_value() {
        &config::String(_) => return Err(CargoError::new(~"The path was configured as a String instead of a List", 1)),
        &config::List(ref list) => list
    };

    println!("Paths: {}: {}", paths.len(), paths);

    let packages: Vec<Package> = paths.iter().filter_map(|path| {
        let joined = Path::new(path.as_slice()).join("Cargo.toml");
        let manifest = cargo_read_manifest(joined.as_str().unwrap());

        match manifest {
            Ok(ref manifest) => Some(Package::from_manifest(manifest)),
            Err(_) => None
        }
    }).collect();

    println!("Packages: {}", packages);
    Ok(())
    //call_rustc(~BufReader::new(manifest_bytes.as_slice()))
}

fn flags<T: FlagConfig + Decodable<FlagDecoder, HammerError>>() -> CargoResult<T> {
    let mut decoder = FlagDecoder::new::<T>(std::os::args().tail());
    Decodable::decode(&mut decoder).to_cargo_error(|e: HammerError| e.message, 1)
}

fn read_manifest(manifest_path: &str) -> CargoResult<Vec<u8>> {
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

    let mut process = try!(Process::configure(config).to_cargo_error(|e: io::IoError| format!("Could not configure process: {}", e), 1));

    input.map(|mut reader| io::util::copy(&mut reader, process.stdin.get_mut_ref()));

    Ok(process)
}

