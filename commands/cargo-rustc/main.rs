#[crate_id="cargo-rustc"];

extern crate toml;

use std::os::args;
use std::io::process::Process;

/**
  cargo-rustc -- ...args

  Delegate ...args to actual rustc command
*/

fn main() {
  let mut arguments = args();
  arguments.shift();

  if arguments[0] != ~"--" {
    fail!("LOL");
  } else {
    arguments.shift();
  }

  match Process::new("rustc", arguments.as_slice()) {
    Ok(mut process) => {
      let stdout = process.stdout.get_mut_ref();
      println!("output: {:s}", stdout.read_to_str().unwrap());

      let stderr = process.stderr.get_mut_ref();
      println!("err: {:s}", stderr.read_to_str().unwrap());
    },
    Err(e) => fail!("Failed to execute: {}", e)
  };
}
