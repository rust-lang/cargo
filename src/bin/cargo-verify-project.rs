#[crate_id="cargo-verify-project"];

extern crate toml;
extern crate getopts;

use std::os::{args,set_exit_status};
use std::io::process::Process;
use getopts::{reqopt,getopts,OptGroup};

/**
  cargo-verify-project --manifest=LOCATION
*/

fn main() {
  let arguments = args();

  let opts = ~[
    reqopt("m", "manifest", "the location of the manifest", "MANIFEST")
  ];

  let matches = match getopts(arguments.tail(), opts) {
    Ok(m) => m,
    Err(err) => {
      fail("missing-argument", "manifest");
      return;
    }
  };

  if !matches.opt_present("m") {
    fail("missing-argument", "manifest");
    return;
  }

  let manifest = matches.opt_str("m").unwrap();
  let file = Path::new(manifest);

  if !file.exists() {
    fail("invalid", "not-found");
    return;
  }

  let root = match toml::parse_from_file(file.as_str().unwrap()) {
    Err(e) => {
      fail("invalid", "invalid-format");
      return;
    },
    Ok(r) => r
  };

  println!("{}", "{ \"success\": \"true\" }");
}

fn fail(reason: &str, value: &str) {
  println!(r#"\{ "{:s}", "{:s}" \}"#, reason, value);
  set_exit_status(1);
}
