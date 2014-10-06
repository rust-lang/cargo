use docopt;

use cargo::core::MultiShell;
use cargo::core::source::{Source, SourceId};
use cargo::sources::git::{GitSource};
use cargo::util::{Config, CliResult, CliError, human, ToUrl};

docopt!(Options, "
Usage:
    cargo git-checkout [options] --url=URL --reference=REF
    cargo git-checkout -h | --help

Options:
    -h, --help              Print this message
    -v, --verbose           Use verbose output
")

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    let Options { flag_url: url, flag_reference: reference, .. } = options;

    let url = try!(url.as_slice().to_url().map_err(|e| {
                       human(format!("The URL `{}` you passed was \
                                      not a valid URL: {}", url, e))
                   })
                   .map_err(|e| CliError::from_boxed(e, 1)));

    let source_id = SourceId::for_git(&url, reference.as_slice(), None);

    let mut config = try!(Config::new(shell, None, None).map_err(|e| {
        CliError::from_boxed(e, 1)
    }));
    let mut source = GitSource::new(&source_id, &mut config);

    try!(source.update().map_err(|e| {
        CliError::new(format!("Couldn't update {}: {}", source, e), 1)
    }));

    Ok(None)
}
