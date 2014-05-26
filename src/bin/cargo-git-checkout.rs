#![crate_id="cargo-git-checkout"]

extern crate cargo;
extern crate serialize;
extern crate hammer;
extern crate url;

use hammer::FlagConfig;
use cargo::{execute_main_without_stdin,CLIResult,CLIError,ToResult};
use cargo::util::ToCLI;
use cargo::sources::git::{GitRemoteRepo,GitRepo};
use url::Url;

#[deriving(Eq,Clone,Decodable)]
struct Options {
    database_path: String,
    checkout_path: String,
    url: String,
    reference: String,
    verbose: bool
}

impl FlagConfig for Options {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options) -> CLIResult<Option<GitRepo>> {
    let Options { database_path, checkout_path, url, reference, verbose } = options;

    let url: Url = try!(from_str(url.as_slice()).to_result(|_|
        CLIError::new(format!("The URL `{}` you passed was not a valid URL", url), None::<&str>, 1)));

    let repo = GitRemoteRepo::new(Path::new(database_path), url, reference, verbose);
    let local = try!(repo.checkout().map_err(|e|
        CLIError::new(format!("Couldn't check out repository: {}", e), None::<&str>, 1)));

    try!(local.copy_to(Path::new(checkout_path)).map_err(|e|
        CLIError::new(format!("Couldn't copy repository: {}", e), None::<&str>, 1)));

    Ok(Some(local))
}
