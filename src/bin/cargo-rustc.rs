#[crate_id="cargo-rustc"];

extern crate toml;
extern crate serialize;
extern crate cargo;

use std::os::args;
use std::io;
use std::io::process::Process;
use serialize::json;
use serialize::{Decoder,Decodable};
use std::path::Path;
use cargo::{Manifest,LibTarget,ExecTarget,Project};

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

  //let mut arguments = args();
  //arguments.shift();

  //if arguments[0] != ~"--" {
    //fail!("LOL");
  //} else {
    //arguments.shift();
  //}

  let Manifest{ root, lib, .. } = manifest;

  let root = Path::new(root);
  let out_dir = lib[0].path;
  let target = join(&root, ~"target");

  let args = ~[
    join(&root, out_dir),
    ~"--out-dir", target,
    ~"--crate-type", ~"lib"
  ];

  io::fs::mkdir_recursive(&root.join("target"), io::UserRWX);

  println!("Executing {}", args);

  let mut p = Process::new("rustc", args).unwrap();
  let o = p.wait_with_output();

  if o.status == std::io::process::ExitStatus(0) {
    println!("output: {:s}", std::str::from_utf8(o.output).unwrap());
  } else {
    fail!("Failed to execute")
  }
}

fn join(path: &Path, part: ~str) -> ~str {
  path.join(part).as_str().unwrap().to_owned()
}
