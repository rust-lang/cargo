use command_prelude::*;

use std::collections::HashMap;
use std::process;
use std::fs::File;
use std::io::Read;

use toml;

use cargo::print_json;

pub fn cli() -> App {
    subcommand("verify-project")
        .about("Check correctness of crate manifest")
        .arg_manifest_path()
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    fn fail(reason: &str, value: &str) -> ! {
        let mut h = HashMap::new();
        h.insert(reason.to_string(), value.to_string());
        print_json(&h);
        process::exit(1)
    }

    let mut contents = String::new();
    let filename = match args.root_manifest(config) {
        Ok(filename) => filename,
        Err(e) => fail("invalid", &e.to_string()),
    };

    let file = File::open(&filename);
    match file.and_then(|mut f| f.read_to_string(&mut contents)) {
        Ok(_) => {}
        Err(e) => fail("invalid", &format!("error reading file: {}", e)),
    };
    if contents.parse::<toml::Value>().is_err() {
        fail("invalid", "invalid-format");
    }

    let mut h = HashMap::new();
    h.insert("success".to_string(), "true".to_string());
    print_json(&h);
    Ok(())
}
