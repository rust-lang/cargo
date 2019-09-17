use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("uninstall")
        .about("Remove a Rust binary")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("spec").multiple(true))
        .arg_package_spec_simple("Package to uninstall")
        .arg(multi_opt("bin", "NAME", "Only uninstall the binary NAME"))
        .arg(opt("root", "Directory to uninstall packages from").value_name("DIR"))
        .after_help(
            "\
The argument SPEC is a package ID specification (see `cargo help pkgid`) to
specify which crate should be uninstalled. By default all binaries are
uninstalled for a crate but the `--bin` and `--example` flags can be used to
only uninstall particular binaries.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let root = args.value_of("root");
    let specs = args
        .values_of("spec")
        .unwrap_or_else(|| args.values_of("package").unwrap_or_default())
        .collect();
    ops::uninstall(root, specs, &values(args, "bin"), config)?;
    Ok(())
}
