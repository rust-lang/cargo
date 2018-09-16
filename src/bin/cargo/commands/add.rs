use command_prelude::*;

use cargo::core::{GitReference, SourceId};
use cargo::ops;
use cargo::util::ToUrl;

pub fn cli() -> App {
    subcommand("add")
        .about("Add a new dependency")
        .arg(Arg::with_name("crate").empty_values(false))
        .arg(
            opt("version", "Specify a version to add from crates.io")
                .alias("vers")
                .value_name("VERSION"),
        )
        .arg(opt("git", "Git URL to add the specified crate from").value_name("URL"))
        .arg(opt("branch", "Branch to use when add from git").value_name("BRANCH"))
        .arg(opt("tag", "Tag to use when add from git").value_name("TAG"))
        .arg(opt("rev", "Specific commit to use when adding from git").value_name("SHA"))
        .arg(opt("path", "Filesystem path to local crate to add").value_name("PATH"))
        .after_help(
            "\
This command allows you to add a dependency to a Cargo.toml manifest file. If <crate> is a github
or gitlab repository URL, or a local path, `cargo add` will try to automatically get the crate name
and set the appropriate `--git` or `--path` value.

Please note that Cargo treats versions like \"1.2.3\" as \"^1.2.3\" (and that \"^1.2.3\" is specified
as \">=1.2.3 and <2.0.0\"). By default, `cargo add` will use this format, as it is the one that the
crates.io registry suggests. One goal of `cargo add` is to prevent you from using wildcard
dependencies (version set to \"*\").",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    let compile_opts = args.compile_options(config, CompileMode::Build)?;

    println!("cargo add subcommand executed");

    let krate = args.value_of("crate")
        .unwrap_or_default();

    println!("crate {:?}", krate);

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

    let version = args.value_of("version");

    ops::add(
            &ws,
            krate,
            &source,
            version,
            &compile_opts,
        )?;

    Ok(())
}
