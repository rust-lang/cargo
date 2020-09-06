use crate::command_prelude::*;
use anyhow::{bail, Context, Result};
use cargo::drop_println;
use cargo::util::format::{Parser, RawChunk};
use serde::Serialize;
use std::fmt;

pub fn cli() -> App {
    subcommand("locate-project")
        .about("Print a JSON representation of a Cargo.toml file's location")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
        .arg(
            opt("format", "Format string used for printing project path")
                .value_name("FORMAT")
                .short("f"),
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

    match args.value_of("format") {
        None => config.shell().print_json(&location),
        Some(format) => print_format(config, &location, format)?,
    }

    Ok(())
}

enum Chunk<'a> {
    Raw(&'a str),
    Root,
}

fn parse_format(format: &str) -> Result<Vec<Chunk<'_>>> {
    let mut chunks = Vec::new();
    for raw in Parser::new(format) {
        chunks.push(match raw {
            RawChunk::Text(text) => Chunk::Raw(text),
            RawChunk::Argument("root") => Chunk::Root,
            RawChunk::Argument(a) => bail!("unsupported pattern `{}`", a),
            RawChunk::Error(err) => bail!("{}", err),
        });
    }
    Ok(chunks)
}

struct Display<'a> {
    format: &'a [Chunk<'a>],
    location: &'a ProjectLocation<'a>,
}

impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.format {
            match chunk {
                Chunk::Raw(s) => f.write_str(s)?,
                Chunk::Root => f.write_str(self.location.root)?,
            }
        }
        Ok(())
    }
}

fn print_format(config: &mut Config, location: &ProjectLocation<'_>, format: &str) -> Result<()> {
    let ref format = parse_format(format)
        .with_context(|| format!("locate-project format `{}` not valid", format))?;

    let display = Display { format, location };
    drop_println!(config, "{}", display);
    Ok(())
}
