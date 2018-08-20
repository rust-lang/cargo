use command_prelude::*;

use std::io::{self, BufRead};

use cargo::core::{Source, SourceId};
use cargo::sources::RegistrySource;
use cargo::util::{CargoError, CargoResultExt};
use cargo::ops;

pub fn cli() -> App {
    subcommand("login")
        .about(
            "Save an api token from the registry locally. \
             If token is not specified, it will be read from stdin.",
        )
        .arg(Arg::with_name("token"))
        .arg(opt("host", "Host to set the token for").value_name("HOST"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    // 1. Get the registry src for this command
    let registry = args.registry(config)?;
    let src_id = match &registry {
        Some(registry)  => SourceId::alt_registry(config, registry),
        None            => SourceId::crates_io(config),
    }?;
    let mut src = RegistrySource::remote(&src_id, config);

    // 2. Update the src so that we have the latest information
    src.update()?;

    // 3. Check if this registry supports cargo login v1
    let reg_cfg = src.config()?.unwrap();
    if !reg_cfg.commands.get("login").unwrap_or(&vec![]).iter().any(|v| v == "v1") {
        let registry = match &registry {
            Some(registry)  => registry,
            None            => "crates-io",
        };
        Err(format_err!("`{}` does not support the `cargo login` command with \
                     version `v1`", registry))?;
    }

    // 4. Either we already have the token, or we get it from the command line
    let token = match args.value_of("token") {
        Some(token) => token.to_string(),
        None => {
            // Print instructions to stdout. The exact wording of the message is determined by
            // whether or not the user passed `--registry`.
            let host = args.value_of("host")
                .map(|s| s.to_string())
                .unwrap_or_else(|| reg_cfg.api.unwrap());
            let separator = match host.ends_with("/") {
                true    => "",
                false   => "/",
            };
            if registry.is_some() {
                println!("please paste the API Token below (you may be able to obtain your token
                          by visiting {}{}me", host, separator);
            } else {
                println!("please visit {}{}me and paste the API Token below", host, separator);
            }

            // Read the token from stdin.
            let mut line = String::new();
            let input = io::stdin();
            input
                .lock()
                .read_line(&mut line)
                .chain_err(|| "failed to read stdin")
                .map_err(CargoError::from)?;
            line.trim().to_string()
        }
    };

    ops::registry_login(config, token, registry)?;
    Ok(())
}
