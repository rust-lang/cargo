use crate::command_prelude::*;

use anyhow::Context;
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
    if version.is_none() {
        return Err(anyhow::format_err!("`--version` is required").into());
    }

    ops::yank(
        gctx,
        krate.map(|s| s.to_string()),
        version.map(|s| s.to_string()),
        args.get_one::<String>("token").cloned().map(Secret::from),
        args.registry_or_index(gctx)?,
        args.flag("undo"),
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

    if let Some(version) = version {
        semver::Version::parse(version).with_context(|| {
            if let Some(stripped) = version.strip_prefix("v") {
                return format!(
                    "the version provided, `{version}` is not a \
                    valid SemVer version\n\n\
                    help: try changing the version to `{stripped}`",
                );
            }
            format!("invalid version `{version}`")
        })?;
    }

    Ok((krate, version))
}
