use crate::command_prelude::*;
use anyhow::bail;
use cargo::{drop_println, CargoResult};
use serde::Serialize;

pub fn cli() -> Command {
    subcommand("locate-project")
        .about("Print a JSON representation of a Cargo.toml file's location")
        .arg(flag("workspace", "Locate Cargo.toml of the workspace root"))
        .arg(
            opt(
                "message-format",
                "Output representation [possible values: json, plain]",
            )
            .value_name("FMT"),
        )
        .arg_quiet()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help locate-project</>` for more detailed information.\n"
        ))
}

#[derive(Serialize)]
pub struct ProjectLocation<'a> {
    root: &'a str,
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let root_manifest;
    let workspace;
    let root = match WhatToFind::parse(args) {
        WhatToFind::CurrentManifest => {
            root_manifest = args.root_manifest(config)?;
            &root_manifest
        }
        WhatToFind::Workspace => {
            workspace = args.workspace(config)?;
            workspace.root_manifest()
        }
    };

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

    match MessageFormat::parse(args)? {
        MessageFormat::Json => config.shell().print_json(&location)?,
        MessageFormat::Plain => drop_println!(config, "{}", location.root),
    }

    Ok(())
}

enum WhatToFind {
    CurrentManifest,
    Workspace,
}

impl WhatToFind {
    fn parse(args: &ArgMatches) -> Self {
        if args.flag("workspace") {
            WhatToFind::Workspace
        } else {
            WhatToFind::CurrentManifest
        }
    }
}

enum MessageFormat {
    Json,
    Plain,
}

impl MessageFormat {
    fn parse(args: &ArgMatches) -> CargoResult<Self> {
        let fmt = match args.get_one::<String>("message-format") {
            Some(fmt) => fmt,
            None => return Ok(MessageFormat::Json),
        };
        match fmt.to_ascii_lowercase().as_str() {
            "json" => Ok(MessageFormat::Json),
            "plain" => Ok(MessageFormat::Plain),
            s => bail!("invalid message format specifier: `{}`", s),
        }
    }
}
