#![crate_id="cargo-git-checkout"]
#![feature(phase)]

extern crate cargo;
extern crate serialize;
extern crate url;

#[phase(plugin, link)]
extern crate hammer;

use cargo::{execute_main_without_stdin};
use cargo::core::MultiShell;
use cargo::core::source::{Source, SourceId};
use cargo::sources::git::{GitSource};
use cargo::util::{Config, CliResult, CliError, Require, human};
use url::Url;

#[deriving(PartialEq,Clone,Decodable)]
struct Options {
    url: String,
    reference: String
}

hammer_config!(Options)

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    let Options { url, reference, .. } = options;

    let url: Url = try!(from_str(url.as_slice())
                        .require(|| human(format!("The URL `{}` you passed was \
                                                   not a valid URL", url)))
                        .map_err(|e| CliError::from_boxed(e, 1)));

    let source_id = SourceId::for_git(&url, reference.as_slice());

    let mut config = try!(Config::new(shell, true, None).map_err(|e| {
        CliError::from_boxed(e, 1)
    }));
    let mut source = GitSource::new(&source_id, &mut config);

    try!(source.update().map_err(|e| {
        CliError::new(format!("Couldn't update {}: {}", source, e), 1)
    }));

    Ok(None)
}
