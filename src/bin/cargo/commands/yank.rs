use crate::command_prelude::*;

use cargo::ops;
use cargo::util::auth::Secret;

pub fn cli() -> Command {
    subcommand("yank")
        .about("Remove a pushed crate from the index")
        .arg_quiet()
        .arg(Arg::new("crate").action(ArgAction::Set))
        .arg(
            opt("version", "The version to yank or un-yank")
                .alias("vers")
                .value_name("VERSION"),
        )
        .arg(flag(
            "undo",
            "Undo a yank, putting a version back into the index",
        ))
        .arg(opt("index", "Registry index to yank from").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help yank` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let registry = args.registry(config)?;

    let (krate, version) = resolve_crate(
        args.get_one::<String>("crate").map(String::as_str),
        args.get_one::<String>("version").map(String::as_str),
    )?;
    if version.is_none() {
        return Err(anyhow::format_err!("`--version` is required").into());
    }

    ops::yank(
        config,
        krate.map(|s| s.to_string()),
        version.map(|s| s.to_string()),
        args.get_one::<String>("token").cloned().map(Secret::from),
        args.get_one::<String>("index").cloned(),
        args.flag("undo"),
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
