use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("uninstall")
        .about("Remove a Rust binary")
        .arg(Arg::new("spec").value_name("SPEC").num_args(0..))
        .arg(opt("root", "Directory to uninstall packages from").value_name("DIR"))
        .arg_quiet()
        .arg_package_spec_simple("Package to uninstall")
        .arg(
            multi_opt("bin", "NAME", "Only uninstall the binary NAME")
                .help_heading(heading::TARGET_SELECTION),
        )
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help uninstall</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let root = args.get_one::<String>("root").map(String::as_str);

    if args.is_present_with_zero_values("package") {
        return Err(anyhow::anyhow!(
            "\"--package <SPEC>\" requires a SPEC format value.\n\
            Run `cargo help pkgid` for more information about SPEC format."
        )
        .into());
    }

    let specs = args
        .get_many::<String>("spec")
        .unwrap_or_else(|| args.get_many::<String>("package").unwrap_or_default())
        .map(String::as_str)
        .collect();
    ops::uninstall(root, specs, &values(args, "bin"), config)?;
    Ok(())
}
