use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("pkgid")
        .about("Print a fully qualified package specification")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("spec"))
        .arg_package("Argument to get the package ID specifier for")
        .arg_manifest_path()
        .after_help(
            "\
Given a <spec> argument, print out the fully qualified package ID specifier.
This command will generate an error if <spec> is ambiguous as to which package
it refers to in the dependency graph. If no <spec> is given, then the pkgid for
the local package is printed.

This command requires that a lockfile is available and dependencies have been
fetched.

Example Package IDs

           pkgid                  |  name  |  version  |          url
    |-----------------------------|--------|-----------|---------------------|
     foo                          | foo    | *         | *
     foo:1.2.3                    | foo    | 1.2.3     | *
     crates.io/foo                | foo    | *         | *://crates.io/foo
     crates.io/foo#1.2.3          | foo    | 1.2.3     | *://crates.io/foo
     crates.io/bar#foo:1.2.3      | foo    | 1.2.3     | *://crates.io/bar
     https://crates.io/foo#1.2.3  | foo    | 1.2.3     | https://crates.io/foo
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let spec = args.value_of("spec").or_else(|| args.value_of("package"));
    let spec = ops::pkgid(&ws, spec)?;
    cargo::drop_println!(config, "{}", spec);
    Ok(())
}
