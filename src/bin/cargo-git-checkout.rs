#![crate_id="cargo-git-checkout"]

extern crate cargo;
extern crate serialize;
extern crate hammer;
extern crate url;

use hammer::FlagConfig;
use cargo::{execute_main_without_stdin,CLIResult,CLIError,ToResult};
use cargo::core::source::Source;
use cargo::sources::git::{GitSource,GitRemote};
use url::Url;

#[deriving(PartialEq,Clone,Decodable)]
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

fn execute(options: Options) -> CLIResult<Option<()>> {
    let Options { database_path, checkout_path, url, reference, verbose } = options;

    let url: Url = try!(from_str(url.as_slice()).to_result(|_|
        CLIError::new(format!("The URL `{}` you passed was not a valid URL", url), None::<&str>, 1)));

    let remote = GitRemote::new(url, verbose);
    let source = GitSource::new(remote, reference, Path::new(database_path), Path::new(checkout_path));
    try!(source.update().map_err(|e| {
        CLIError::new(format!("Couldn't update {}: {}", source, e), None::<&str>, 1)
    }));

    Ok(None)
}
