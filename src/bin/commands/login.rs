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
    let registry = args.registry(config)?;

    let token = match args.value_of("token") {
        Some(token) => token.to_string(),
        None => {
            let host = match registry {
                Some(ref _registry) => {
                    return Err(format_err!(
                        "token must be provided when \
                         --registry is provided."
                    ).into());
                }
                None => {
                    let src = SourceId::crates_io(config)?;
                    let mut src = RegistrySource::remote(&src, config);
                    src.update()?;
                    let config = src.config()?.unwrap();
                    args.value_of("host")
                        .map(|s| s.to_string())
                        .unwrap_or(config.api.unwrap())
                }
            };
            println!("please visit {}me and paste the API Token below", host);
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
