use command_prelude::*;

use cargo::core::{GitReference, SourceId};
use cargo::ops;
use cargo::util::ToUrl;

pub fn cli() -> App {
    subcommand("add")
        .about("Add a new dependency")
        .arg(Arg::with_name("crate").empty_values(false))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let mut compile_opts = args.compile_options(config, CompileMode::Build)?;
    compile_opts.build_config.release = !args.is_present("debug");

    println!("cargo add subcommand executed");

    let krate = args.value_of("crate")
        .unwrap_or_default();

    println!("crate {:?}", krate);

    let mut from_cwd = false;

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
    } else if krate.is_empty() {
        from_cwd = true;
        SourceId::for_path(config.cwd())?
    } else {
        SourceId::crates_io(config)?
    };

    let version = args.value_of("version");
    let root = args.value_of("root");

    ops::add(
            root,
            krate,
            &source,
            from_cwd,
            version,
            &compile_opts,
            args.is_present("force"),
        )?;

    Ok(())
}
