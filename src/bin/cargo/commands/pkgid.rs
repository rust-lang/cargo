use crate::command_prelude::*;

use cargo::ops;
use cargo::util::print_available_packages;

pub fn cli() -> App {
    subcommand("pkgid")
        .about("Print a fully qualified package specification")
        .arg_quiet()
        .arg(Arg::new("spec"))
        .arg_package("Argument to get the package ID specifier for")
        .arg_manifest_path()
        .after_help("Run `cargo help pkgid` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?
    }
    let spec = args.value_of("spec").or_else(|| args.value_of("package"));
    let spec = ops::pkgid(&ws, spec)?;
    cargo::drop_println!(config, "{}", spec);
    Ok(())
}
