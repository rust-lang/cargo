#![crate_name="cargo-build"]
#![feature(phase)]

extern crate cargo;

#[phase(plugin, link)]
extern crate hammer;

#[phase(plugin, link)]
extern crate log;

extern crate serialize;

use std::os;
use cargo::{execute_main_without_stdin};
use cargo::ops;
use cargo::ops::CompileOptions;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[deriving(PartialEq,Clone,Decodable,Encodable)]
pub struct Options {
    manifest_path: Option<String>,
    update_remotes: bool,
    jobs: Option<uint>,
    target: Option<String>,
    release: bool,
}

hammer_config!(Options "Build the current project", |c| {
    c.short("update_remotes", 'u')
     .short("jobs", 'j')
})

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-compile; args={}", os::args());

    let root = try!(find_root_manifest_for_cwd(options.manifest_path));

    let env = if options.release {
        "release"
    } else {
        "compile"
    };

    let mut opts = CompileOptions {
        update: options.update_remotes,
        env: env,
        shell: shell,
        jobs: options.jobs,
        target: options.target.as_ref().map(|t| t.as_slice()),
    };

    ops::compile(&root, &mut opts).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
