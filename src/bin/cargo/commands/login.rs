use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("login")
        .about(
            "Save an api token from the registry locally. \
             If token is not specified, it will be read from stdin.",
        )
        .arg_quiet()
        .arg(Arg::new("token").action(ArgAction::Set))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .arg(
            flag(
                "generate-keypair",
                "Generate a public/secret keypair (unstable)",
            )
            .conflicts_with("token"),
        )
        .arg(
            flag("secret-key", "Prompt for secret key (unstable)")
                .conflicts_with_all(&["generate-keypair", "token"]),
        )
        .arg(
            opt(
                "key-subject",
                "Set the key subject for this registry (unstable)",
            )
            .value_name("SUBJECT")
            .conflicts_with("token"),
        )
        .after_help("Run `cargo help login` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let registry = args.registry(config)?;
    ops::registry_login(
        config,
        args.get_one::<String>("token").map(|s| s.as_str().into()),
        registry.as_deref(),
        args.flag("generate-keypair"),
        args.flag("secret-key"),
        args.get_one("key-subject").map(String::as_str),
    )?;
    Ok(())
}
