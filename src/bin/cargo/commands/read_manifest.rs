use crate::command_prelude::*;

pub fn cli() -> App {
    subcommand("read-manifest")
        .about(
            "\
Print a JSON representation of a Cargo.toml manifest.

Deprecated, use `cargo metadata --no-deps` instead.\
",
        )
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    config.shell().print_json(&ws.current()?.serialized(config));
    Ok(())
}
