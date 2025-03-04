use crate::command_prelude::*;

use cargo::ops;
use cargo_credential::Secret;

pub fn cli() -> Command {
    subcommand("yank")
        .about("Remove a pushed crate from the index")
        .arg(Arg::new("crate").value_name("CRATE").action(ArgAction::Set))
        .arg(
            opt("version", "The version to yank or un-yank")
                .alias("vers")
                .value_name("VERSION"),
        )
        .arg(flag(
            "undo",
            "Undo a yank, putting a version back into the index",
        ))
        .arg_index("Registry index URL to yank from")
        .arg_registry("Registry to yank from")
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg_silent_suggestion()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help yank</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let (krate, version) = resolve_crate(
        args.get_one::<String>("crate").map(String::as_str),
        args.get_one::<String>("version").map(String::as_str),
    )?;

    ops::yank(
        gctx,
        krate.map(|s| s.to_string()),
        version.to_string(),
        args.get_one::<String>("token").cloned().map(Secret::from),
        args.registry_or_index(gctx)?,
        args.flag("undo"),
    )?;
    Ok(())
}

fn resolve_crate<'k>(
    krate: Option<&'k str>,
    version: Option<&'k str>,
) -> crate::CargoResult<(Option<&'k str>, &'k str)> {
    match krate.and_then(|k| k.split_once('@')) {
        Some((name, embedded_version)) => {
            if name.is_empty() {
                // by convention, arguments starting with `@` are response files
                anyhow::bail!("missing crate name for `@{embedded_version}`");
            }

            match version {
                None => Ok((Some(name), embedded_version)),
                Some(_) => {
                    anyhow::bail!("cannot specify both `@{embedded_version}` and `--version`");
                }
            }
        }
        None => match version {
            Some(version) => Ok((krate, version)),
            None => {
                anyhow::bail!("`--version` is required");
            }
        },
    }
}
