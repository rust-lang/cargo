use command_prelude::*;

use cargo::core::{GitReference, SourceId};
use cargo::util::ToUrl;
use cargo::ops::{self, CloneOptions};

pub fn cli() -> App {
    subcommand("clone")
        .about("Clone source code of a Rust crate")
        .arg(opt("prefix", "Directory to clone the package into").value_name("DIR"))
        .arg(opt("force", "Force overwriting existing directory").short("f"))
        .arg(Arg::with_name("crate").empty_values(false))
        .arg(opt("version", "Specify a version to install from crates.io")
                .alias("vers")
                .value_name("VERSION"),
        )
        .arg(opt("git", "Git URL to clone the specified crate from").value_name("URL"))
        .arg(opt("branch", "Branch to use when cloning from git").value_name("BRANCH"))
        .arg(opt("tag", "Tag to use when cloning from git").value_name("TAG"))
        .arg(opt("rev", "Specific commit to use when cloning from git").value_name("SHA"))
        .arg(opt("path", "Filesystem path to local crate to clone").value_name("PATH"))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let source = if let Some(url) = args.value_of("git") {
        let url = url.to_url()?;
        let gitref = if let Some(branch) = args.value_of("branch") {
            GitReference::Branch(branch.to_string())
        } else if let Some(tag) = args.value_of("tag") {
            GitReference::Tag(tag.to_string())
        } else if let Some(rev) = args.value_of("rev") {
            GitReference::Rev(rev.to_string())
        } else {
            GitReference::Branch("master".to_string())
        };
        SourceId::for_git(&url, gitref)?
    } else if let Some(path) = args.value_of_path("path", config) {
        SourceId::for_path(&path)?
    } else {
        SourceId::crates_io(config)?
    };

    let opts = CloneOptions {
        config,
        name: args.value_of("crate"),
        source_id: source,
        prefix: args.value_of("prefix"),
        force: args.is_present("force"),
        version: args.value_of("version"),
    };
    ops::clone(opts)?;
    Ok(())
}