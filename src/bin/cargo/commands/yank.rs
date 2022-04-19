use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("yank")
        .about("Remove a pushed crate from the index")
        .arg_quiet()
        .arg(Arg::new("crate"))
        .arg(
            opt("version", "The version to yank or un-yank")
                .alias("vers")
                .value_name("VERSION"),
        )
        .arg(opt(
            "undo",
            "Undo a yank, putting a version back into the index",
        ))
        .arg(opt("index", "Registry index to yank from").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help yank` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    config.load_credentials()?;

    let registry = args.registry(config)?;

    let (krate, version) = resolve_crate(args.value_of("crate"), args.value_of("version"))?;
    if version.is_none() {
        return Err(anyhow::format_err!("`--version` is required").into());
    }

    ops::yank(
        config,
        krate.map(|s| s.to_string()),
        version.map(|s| s.to_string()),
        args.value_of("token").map(|s| s.to_string()),
        args.value_of("index").map(|s| s.to_string()),
        args.is_present("undo"),
        registry,
    )?;
    Ok(())
}

fn resolve_crate<'k>(
    mut krate: Option<&'k str>,
    mut version: Option<&'k str>,
) -> crate::CargoResult<(Option<&'k str>, Option<&'k str>)> {
    if let Some((k, v)) = krate.and_then(|k| k.split_once('@')) {
        if version.is_some() {
            anyhow::bail!("cannot specify both `@{v}` and `--version`");
        }
        if k.is_empty() {
            // by convention, arguments starting with `@` are response files
            anyhow::bail!("missing crate name for `@{v}`");
        }
        krate = Some(k);
        version = Some(v);
    }
    Ok((krate, version))
}
