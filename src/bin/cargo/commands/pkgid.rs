use crate::command_prelude::*;

use cargo::ops;
use cargo::util::print_available_packages;

pub fn cli() -> Command {
    subcommand("pkgid")
        .about("Print a fully qualified package specification")
        .arg(Arg::new("spec").value_name("SPEC").action(ArgAction::Set))
        .arg_silent_suggestion()
        .arg_package("Argument to get the package ID specifier for")
        .arg_manifest_path()
        .arg_lockfile_path()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help pkgid</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(gctx)?;
    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?
    }
    let spec = args
        .get_one::<String>("spec")
        .or_else(|| args.get_one::<String>("package"))
        .map(String::as_str);
    let spec = ops::pkgid(&ws, spec)?;
    cargo::drop_println!(gctx, "{}", spec);
    Ok(())
}
