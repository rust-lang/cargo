#![crate_id="cargo-verify-project"]

extern crate toml = "github.com/mneumann/rust-toml#toml";
extern crate getopts;

use std::os::{args,set_exit_status};
use getopts::{reqopt,getopts};

/**
    cargo-verify-project --manifest=LOCATION
*/

fn main() {
    let arguments = args();

    let opts = vec!(
        reqopt("m", "manifest", "the location of the manifest", "MANIFEST")
    );

    let matches = match getopts(arguments.tail(), opts.as_slice()) {
        Ok(m) => m,
        Err(_) => {
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

    match toml::parse_from_file(file.as_str().unwrap()) {
        Err(_) => {
            fail("invalid", "invalid-format");
            return;
        },
        Ok(r) => r
    };

    println!("{}", "{ \"success\": \"true\" }");
}

fn fail(reason: &str, value: &str) {
    println!(r#"{{ "{:s}", "{:s}" }}"#, reason, value);
    set_exit_status(1);
}
