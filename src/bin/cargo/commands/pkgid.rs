use crate::command_prelude::*;

use cargo::ops;
use cargo::util::print_available_packages;

pub fn cli() -> Command {
    subcommand("pkgid")
        .about("Print a fully qualified package specification")
        .arg(Arg::new("spec").value_name("SPEC").action(ArgAction::Set))
        .arg_quiet()
        .arg_package("Argument to get the package ID specifier for")
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help pkgid</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    if ws.root_maybe().is_embedded() {
        return Err(anyhow::format_err!(
            "{} is unsupported by `cargo pkgid`",
            ws.root_manifest().display()
        )
        .into());
    }
    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?
    }
    let spec = args
        .get_one::<String>("spec")
        .or_else(|| args.get_one::<String>("package"))
        .map(String::as_str);
    let spec = ops::pkgid(&ws, spec)?;
    cargo::drop_println!(config, "{}", spec);
    Ok(())
}
