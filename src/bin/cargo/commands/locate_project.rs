use command_prelude::*;

use cargo::print_json;

pub fn cli() -> App {
    subcommand("locate-project")
        .about("Print a JSON representation of a Cargo.toml file's location")
        .arg_manifest_path()
}

#[derive(Serialize)]
pub struct ProjectLocation<'a> {
    root: &'a str,
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let root = args.root_manifest(config)?;

    let root = root.to_str()
        .ok_or_else(|| {
            format_err!(
                "your project path contains characters \
                 not representable in Unicode"
            )
        })
        .map_err(|e| CliError::new(e, 1))?;

    let location = ProjectLocation { root };

    print_json(&location);
    Ok(())
}
