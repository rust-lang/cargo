use cargo::core::source::{Source, SourceId, GitReference};
use cargo::sources::git::{GitSource};
use cargo::util::{Config, CliResult, CliError, human, ToUrl};

#[derive(RustcDecodable)]
pub struct Options {
    flag_url: String,
    flag_reference: String,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
}

pub const USAGE: &'static str = "
Checkout a copy of a Git repository

Usage:
    cargo git-checkout [options] --url=URL --reference=REF
    cargo git-checkout -h | --help

Options:
    -h, --help               Print this message
    -v, --verbose ...        Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.configure_shell(options.flag_verbose,
                                options.flag_quiet,
                                &options.flag_color));
    let Options { flag_url: url, flag_reference: reference, .. } = options;

    let url = try!(url.to_url().map_err(|e| {
                       human(format!("The URL `{}` you passed was \
                                      not a valid URL: {}", url, e))
                   })
                   .map_err(|e| CliError::new(e, 1)));

    let reference = GitReference::Branch(reference.clone());
    let source_id = SourceId::for_git(&url, reference);

    let mut source = GitSource::new(&source_id, config);

    try!(source.update().map_err(|e| {
        CliError::new(human(format!("Couldn't update {:?}: {:?}", source, e)), 1)
    }));

    Ok(None)
}
