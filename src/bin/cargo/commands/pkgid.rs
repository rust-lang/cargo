use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("pkgid")
        .about("Print a fully qualified package specification")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("spec"))
        .arg_package("Argument to get the package ID specifier for")
        .arg_manifest_path()
        .after_help("Run `cargo help pkgid` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let spec = args.value_of("spec").or_else(|| args.value_of("package"));
    let spec = ops::pkgid(&ws, spec)?;
    cargo::drop_println!(config, "{}", spec);
    Ok(())
}
