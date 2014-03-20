#[crate_id="cargo-rustc"];
#[allow(deprecated_owned_vector)];

extern crate toml;
extern crate serialize;
extern crate cargo;

use std::os::args;
use std::io;
use std::io::process::{Process,ProcessConfig,InheritFd};
use serialize::json;
use serialize::Decodable;
use std::path::Path;
use cargo::{Manifest,CargoResult,CargoError,ToCargoError};

/**
    cargo-rustc -- ...args

    Delegate ...args to actual rustc command
*/

fn main() {
    match execute() {
        Err(e) => {
            write!(&mut std::io::stderr(), "{}", e.message);
            // TODO: Exit with error code
        },
        _ => return
    }
}

fn execute() -> CargoResult<()> {
    let mut reader = io::stdin();
    let input = try!(reader.read_to_str().to_cargo_error(~"Cannot read stdin to a string", 1));

    let json = try!(json::from_str(input).to_cargo_error(format!("Cannot parse json: {}", input), 1));
    let mut decoder = json::Decoder::new(json);
    let manifest: Manifest = Decodable::decode(&mut decoder);

    let Manifest{ root, lib, bin, .. } = manifest;

    let (crate_type, out_dir) = if lib.len() > 0 {
        ( ~"lib", lib[0].path )
    } else if bin.len() > 0 {
        ( ~"bin", bin[0].path )
    } else {
        return Err(CargoError::new(~"bad manifest, no lib or bin specified", 1));
    };

    let root = Path::new(root);
    let target = join(&root, ~"target");

    let args = [
        join(&root, out_dir),
        ~"--out-dir", target,
        ~"--crate-type", crate_type
    ];

    match io::fs::mkdir_recursive(&root.join("target"), io::UserRWX) {
        Err(_) => fail!("Couldn't mkdir -p"),
        Ok(val) => val
    }

    println!("Executing rustc {}", args.as_slice());

    let mut config = ProcessConfig::new();
    config.stdout = InheritFd(1);
    config.stderr = InheritFd(2);
    config.program = "rustc";
    config.args = args.as_slice();

    let mut p = try!(Process::configure(config).to_cargo_error(format!("Could not start process: rustc {}", args.as_slice()), 1));

    let status = p.wait();

    if status != std::io::process::ExitStatus(0) {
        fail!("Failed to execute")
    }

    Ok(())
}

fn join(path: &Path, part: ~str) -> ~str {
    format!("{}", path.join(part).display())
}

fn vec_idx<T>(v: ~[T], idx: uint) -> Option<T> {
    if idx < v.len() {
        Some(v[idx])
    } else {
        None
    }
}
