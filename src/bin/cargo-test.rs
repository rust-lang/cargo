#![crate_name="cargo-test"]
#![feature(phase)]

#[phase(plugin, link)]
extern crate cargo;
extern crate serialize;

#[phase(plugin, link)]
extern crate hammer;

use std::os;
use std::io::{UserExecute, fs};

use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell};
use cargo::util;
use cargo::util::{CliResult, CliError, CargoError};
use cargo::util::important_paths::find_project_manifest;

#[deriving(PartialEq,Clone,Decodable)]
struct Options {
    manifest_path: Option<String>,
    jobs: Option<uint>,
    rest: Vec<String>,
}

hammer_config!(Options "Run the package's test suite", |c| {
    c.short("jobs", 'j')
})

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    let root = match options.manifest_path {
        Some(path) => Path::new(path),
        None => try!(find_project_manifest(&os::getcwd(), "Cargo.toml")
                    .map_err(|_| {
                        CliError::new("Could not find Cargo.toml in this \
                                       directory or any parent directory",
                                      102)
                    }))
    };

    let compile_opts = ops::CompileOptions {
        update: false,
        env: "test",
        shell: shell,
        jobs: options.jobs
    };

    try!(ops::compile(&root, compile_opts).map(|_| None::<()>).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));

    let test_dir = root.dir_path().join("target").join("test");

    let mut walk = try!(fs::walk_dir(&test_dir).map_err(|e| {
        CliError::from_error(e, 1)
    }));

    for file in walk {
        // TODO: The proper fix is to have target knows its expected
        // output and only run expected executables.
        if file.display().to_string().as_slice().contains("dSYM") { continue; }
        if !is_executable(&file) { continue; }

        try!(util::process(file).exec().map_err(|e| {
            CliError::from_boxed(e.box_error(), 1)
        }));
    }

    Ok(None)
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() { return false; }
    path.stat().map(|stat| stat.perm.intersects(UserExecute)).unwrap_or(false)
}
