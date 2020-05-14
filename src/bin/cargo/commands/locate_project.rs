use crate::command_prelude::*;
use serde::Serialize;

pub fn cli() -> App {
    subcommand("locate-project")
        .about("Print a JSON representation of a Cargo.toml file's location")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
}

#[derive(Serialize)]
pub struct ProjectLocation<'a> {
    root: &'a str,
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let root = args.root_manifest(config)?;

    let root = root
        .to_str()
        .ok_or_else(|| {
            anyhow::format_err!(
                "your package path contains characters \
                 not representable in Unicode"
            )
        })
        .map_err(|e| CliError::new(e, 1))?;

    let location = ProjectLocation { root };

    config.shell().print_json(&location);
    Ok(())
}
