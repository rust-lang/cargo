use command_prelude::*;

use cargo::core::Package;
use cargo::print_json;

pub fn cli() -> App {
    subcommand("read-manifest")
        .about("Deprecated, use `cargo metadata --no-deps` instead.
Print a JSON representation of a Cargo.toml manifest.")
        .arg_manifest_path()
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let root = args.root_manifest(config)?;
    let pkg = Package::for_path(&root, config)?;
    print_json(&pkg);
    Ok(())
}
