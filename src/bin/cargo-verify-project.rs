#![crate_name="cargo-verify-project"]

extern crate toml;
extern crate getopts;

use std::io::File;
use std::os::{args, set_exit_status};
use getopts::{reqopt, getopts};

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

    let manifest = match matches.opt_str("m") {
        Some(m) => m,
        None => {
            fail("missing-argument", "manifest");
            return;
        }
    };
    let file = Path::new(manifest);
    let contents = match File::open(&file).read_to_string() {
        Ok(s) => s,
        Err(e) => return fail("invalid", format!("error reading file: {}",
                                                 e).as_slice())
    };
    match toml::Parser::new(contents.as_slice()).parse() {
        None => {
            fail("invalid", "invalid-format");
            return;
        },
        Some(..) => {}
    };

    println!("{}", "{ \"success\": \"true\" }");
}

fn fail(reason: &str, value: &str) {
    println!(r#"{{ "{:s}": "{:s}" }}"#, reason, value);
    set_exit_status(1);
}
