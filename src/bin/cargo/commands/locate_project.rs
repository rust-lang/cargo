use crate::command_prelude::*;
use anyhow::bail;
use cargo::{drop_println, CargoResult};
use serde::Serialize;

pub fn cli() -> App {
    subcommand("locate-project")
        .about("Print a JSON representation of a Cargo.toml file's location")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
        .arg(
            opt(
                "message-format",
                "Output representation [possible values: json, plain]",
            )
            .value_name("FMT"),
        )
        .after_help("Run `cargo help locate-project` for more detailed information.\n")
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

    match MessageFormat::parse(args)? {
        MessageFormat::Json => config.shell().print_json(&location),
        MessageFormat::Plain => drop_println!(config, "{}", location.root),
    }

    Ok(())
}

enum MessageFormat {
    Json,
    Plain,
}

impl MessageFormat {
    fn parse(args: &ArgMatches<'_>) -> CargoResult<Self> {
        let fmt = match args.value_of("message-format") {
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
