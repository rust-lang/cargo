#![crate_id="cargo-git-checkout"]

extern crate cargo;
extern crate serialize;
extern crate hammer;
extern crate url;

use hammer::FlagConfig;
use cargo::{execute_main_without_stdin,CLIResult,CLIError,ToResult};
use cargo::util::ToCLI;
use cargo::sources::git::{GitCommand,GitRepo};
use url::Url;

#[deriving(Eq,Clone,Decodable)]
struct Options {
    directory: String,
    url: String,
    reference: String
}

impl FlagConfig for Options {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options) -> CLIResult<Option<GitRepo>> {
    let url: Url = try!(from_str(options.url.as_slice()).to_result(|_|
        CLIError::new(format!("The URL `{}` you passed was not a valid URL", options.url), None::<&str>, 1)));

    let cmd = GitCommand::new(Path::new(options.directory.clone()), url, options.reference);
    cmd.checkout().to_cli(1).map(|repo| Some(repo))
}
