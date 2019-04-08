use crate::command_prelude::*;

use cargo::core::{GitReference, Source, SourceId};
use cargo::sources::GitSource;
use cargo::util::ToUrl;

pub fn cli() -> App {
    subcommand("git-checkout")
        .about("Checkout a copy of a Git repository")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(
            Arg::with_name("url")
                .long("url")
                .value_name("URL")
                .required(true),
        )
        .arg(
            Arg::with_name("reference")
                .long("reference")
                .value_name("REF")
                .required(true),
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let url = args.value_of("url").unwrap().to_url()?;
    let reference = args.value_of("reference").unwrap();

    let reference = GitReference::Branch(reference.to_string());
    let source_id = SourceId::for_git(&url, reference)?;

    let mut source = GitSource::new(source_id, config)?;

    source.update()?;

    Ok(())
}
