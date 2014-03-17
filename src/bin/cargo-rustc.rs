#[crate_id="cargo-rustc"];

extern crate toml;
extern crate serialize;
extern crate cargo;

use std::os::args;
use std::io;
use std::io::process::{Process,ProcessConfig,InheritFd};
use serialize::json;
use serialize::Decodable;
use std::path::Path;
use cargo::Manifest;

/**
    cargo-rustc -- ...args

    Delegate ...args to actual rustc command
*/

fn main() {
    let mut reader = io::stdin();
    let input = reader.read_to_str().unwrap();

    let json = json::from_str(input).unwrap();
    let mut decoder = json::Decoder::new(json);
    let manifest: Manifest = Decodable::decode(&mut decoder);

    let Manifest{ root, lib, .. } = manifest;

    let root = Path::new(root);
    let out_dir = lib[0].path;
    let target = join(&root, ~"target");

    let args = [
        join(&root, out_dir),
        ~"--out-dir", target,
        ~"--crate-type", ~"lib"
    ];

    match io::fs::mkdir_recursive(&root.join("target"), io::UserRWX) {
        Err(_) => fail!("Couldn't mkdir -p"),
        Ok(val) => val
    }

    println!("Executing {}", args.as_slice());

    let mut config = ProcessConfig::new();
    config.stdout = InheritFd(1);
    config.stderr = InheritFd(2);
    config.program = "rustc";
    config.args = args.as_slice();

    let mut p = Process::configure(config).unwrap();

    let status = p.wait();

    if status != std::io::process::ExitStatus(0) {
        fail!("Failed to execute")
    }
}

fn join(path: &Path, part: ~str) -> ~str {
    path.join(part).as_str().unwrap().to_owned()
}
