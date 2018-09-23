use command_prelude::*;

use cargo::ops;

use super::install;

pub fn cli() -> App {
    subcommand("add")
        .about("Add a new dependency")
        .arg(Arg::with_name("crate").empty_values(false).multiple(true))
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

    let krates = args.values_of("crate")
        .unwrap_or_default()
        .collect::<Vec<_>>();

    println!("crate {:?}", krates);

    let (_from_cwd, source) = install::get_source_id(&config, &args, &krates)?;

    let version = args.value_of("version");

    ops::add(
            &ws,
            krates,
            &source,
            version,
            &compile_opts,
        )?;

    Ok(())
}
