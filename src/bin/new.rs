use std::os;
use rustc_serialize::{Decodable, Decoder};

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};

#[deriving(Show, PartialEq)]
enum VersionControl { Git, Hg, NoVcs }

impl<E, D: Decoder<E>> Decodable<D, E> for VersionControl {
    fn decode(d: &mut D) -> Result<VersionControl, E> {
        Ok(match try!(d.read_str()).as_slice() {
            "git" => VersionControl::Git,
            "hg" => VersionControl::Hg,
            "none" => VersionControl::NoVcs,
            n => {
                let err = format!("could not decode '{}' as version control", n);
                return Err(d.error(err.as_slice()));
            }
        })
    }
}

#[deriving(RustcDecodable)]
struct Options {
    flag_verbose: bool,
    flag_bin: bool,
    flag_travis: bool,
    arg_path: String,
    flag_vcs: Option<VersionControl>,
}

pub const USAGE: &'static str = "
Create a new cargo package at <path>

Usage:
    cargo new [options] <path>
    cargo new -h | --help

Options:
    -h, --help          Print this message
    --vcs <vcs>         Initialize a new repository for the given version
                        control system (git or hg) or do not initialize any version 
                        control at all (none) overriding a global configuration. 
    --travis            Create a .travis.yml file
    --bin               Use a binary instead of a library template
    -v, --verbose       Use verbose output
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={}", os::args());
    shell.set_verbose(options.flag_verbose);

    let Options { flag_travis, flag_bin, arg_path, flag_vcs, .. } = options;

    let opts = ops::NewOptions {
        no_git: flag_vcs == Some(VersionControl::NoVcs),
        git: flag_vcs == Some(VersionControl::Git),
        hg: flag_vcs == Some(VersionControl::Hg),
        travis: flag_travis,
        path: arg_path.as_slice(),
        bin: flag_bin,
    };

    ops::new(opts, shell).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}


