use crate::command_prelude::*;

pub fn cli() -> App {
    subcommand("read-manifest")
        .about(
            "\
Print a JSON representation of a Cargo.toml manifest.

Deprecated, use `cargo metadata --no-deps` instead.\
",
        )
        .arg_quiet()
        .arg_manifest_path()
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    config.shell().print_json(&ws.current()?.serialized())?;
    Ok(())
}
