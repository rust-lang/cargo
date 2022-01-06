use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("uninstall")
        .about("Remove a Rust binary")
        .arg_quiet()
        .arg(Arg::new("spec").multiple_values(true))
        .arg_package_spec_simple("Package to uninstall")
        .arg(multi_opt("bin", "NAME", "Only uninstall the binary NAME"))
        .arg(opt("root", "Directory to uninstall packages from").value_name("DIR"))
        .after_help("Run `cargo help uninstall` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let root = args.value_of("root");

    if args.is_present_with_zero_values("package") {
        return Err(anyhow::anyhow!(
            "\"--package <SPEC>\" requires a SPEC format value.\n\
            Run `cargo help pkgid` for more information about SPEC format."
        )
        .into());
    }

    let specs = args
        .values_of("spec")
        .unwrap_or_else(|| args.values_of("package").unwrap_or_default())
        .collect();
    ops::uninstall(root, specs, &values(args, "bin"), config)?;
    Ok(())
}
