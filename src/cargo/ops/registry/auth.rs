//! Registry authentication support.

use crate::sources::CRATES_IO_REGISTRY;
use crate::util::{config, CargoResult, Config};
use anyhow::{bail, format_err, Context as _};
use cargo_util::ProcessError;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

enum Action {
    Get,
    Store(String),
    Erase,
}

/// Returns the token to use for the given registry.
pub(super) fn auth_token(
    config: &Config,
    cli_token: Option<&str>,
    config_token: Option<&str>,
    credential_process: Option<&(PathBuf, Vec<String>)>,
    registry_name: Option<&str>,
    api_url: &str,
) -> CargoResult<String> {
    let token = match (cli_token, config_token, credential_process) {
        (None, None, None) => {
            bail!("no upload token found, please run `cargo login` or pass `--token`");
        }
        (Some(cli_token), _, _) => cli_token.to_string(),
        (None, Some(config_token), _) => config_token.to_string(),
        (None, None, Some(process)) => {
            let registry_name = registry_name.unwrap_or(CRATES_IO_REGISTRY);
            run_command(config, process, registry_name, api_url, Action::Get)?.unwrap()
        }
    };
    Ok(token)
}

/// Saves the given token.
pub(super) fn login(
    config: &Config,
    token: String,
    credential_process: Option<&(PathBuf, Vec<String>)>,
    registry_name: Option<&str>,
    api_url: &str,
) -> CargoResult<()> {
    if let Some(process) = credential_process {
        let registry_name = registry_name.unwrap_or(CRATES_IO_REGISTRY);
        run_command(
            config,
            process,
            registry_name,
            api_url,
            Action::Store(token),
        )?;
    } else {
        config::save_credentials(config, Some(token), registry_name)?;
    }
    Ok(())
}

/// Removes the token for the given registry.
pub(super) fn logout(
    config: &Config,
    credential_process: Option<&(PathBuf, Vec<String>)>,
    registry_name: Option<&str>,
    api_url: &str,
) -> CargoResult<()> {
    if let Some(process) = credential_process {
        let registry_name = registry_name.unwrap_or(CRATES_IO_REGISTRY);
        run_command(config, process, registry_name, api_url, Action::Erase)?;
    } else {
        config::save_credentials(config, None, registry_name)?;
    }
    Ok(())
}

fn run_command(
    config: &Config,
    process: &(PathBuf, Vec<String>),
    name: &str,
    api_url: &str,
    action: Action,
) -> CargoResult<Option<String>> {
    let cred_proc;
    let (exe, args) = if process.0.to_str().unwrap_or("").starts_with("cargo:") {
        cred_proc = sysroot_credential(config, process)?;
        &cred_proc
    } else {
        process
    };
    if !args.iter().any(|arg| arg.contains("{action}")) {
        let msg = |which| {
            format!(
                "credential process `{}` cannot be used to {}, \
                 the credential-process configuration value must pass the \
                 `{{action}}` argument in the config to support this command",
                exe.display(),
                which
            )
        };
        match action {
            Action::Get => {}
            Action::Store(_) => bail!(msg("log in")),
            Action::Erase => bail!(msg("log out")),
        }
    }
    let action_str = match action {
        Action::Get => "get",
        Action::Store(_) => "store",
        Action::Erase => "erase",
    };
    let args: Vec<_> = args
        .iter()
        .map(|arg| {
            arg.replace("{action}", action_str)
                .replace("{name}", name)
                .replace("{api_url}", api_url)
        })
        .collect();

    let mut cmd = Command::new(&exe);
    cmd.args(args)
        .env("CARGO", config.cargo_exe()?)
        .env("CARGO_REGISTRY_NAME", name)
        .env("CARGO_REGISTRY_API_URL", api_url);
    match action {
        Action::Get => {
            cmd.stdout(Stdio::piped());
        }
        Action::Store(_) => {
            cmd.stdin(Stdio::piped());
        }
        Action::Erase => {}
    }
    let mut child = cmd.spawn().with_context(|| {
        let verb = match action {
            Action::Get => "fetch",
            Action::Store(_) => "store",
            Action::Erase => "erase",
        };
        format!(
            "failed to execute `{}` to {} authentication token for registry `{}`",
            exe.display(),
            verb,
            name
        )
    })?;
    let mut token = None;
    match &action {
        Action::Get => {
            let mut buffer = String::new();
            log::debug!("reading into buffer");
            child
                .stdout
                .as_mut()
                .unwrap()
                .read_to_string(&mut buffer)
                .with_context(|| {
                    format!(
                        "failed to read token from registry credential process `{}`",
                        exe.display()
                    )
                })?;
            if let Some(end) = buffer.find('\n') {
                if buffer.len() > end + 1 {
                    bail!(
                        "credential process `{}` returned more than one line of output; \
                         expected a single token",
                        exe.display()
                    );
                }
                buffer.truncate(end);
            }
            token = Some(buffer);
        }
        Action::Store(token) => {
            writeln!(child.stdin.as_ref().unwrap(), "{}", token).with_context(|| {
                format!(
                    "failed to send token to registry credential process `{}`",
                    exe.display()
                )
            })?;
        }
        Action::Erase => {}
    }
    let status = child.wait().with_context(|| {
        format!(
            "registry credential process `{}` exit failure",
            exe.display()
        )
    })?;
    if !status.success() {
        let msg = match action {
            Action::Get => "failed to authenticate to registry",
            Action::Store(_) => "failed to store token to registry",
            Action::Erase => "failed to erase token from registry",
        };
        return Err(ProcessError::new(
            &format!(
                "registry credential process `{}` {} `{}`",
                exe.display(),
                msg,
                name
            ),
            Some(status),
            None,
        )
        .into());
    }
    Ok(token)
}

/// Gets the path to the libexec processes in the sysroot.
fn sysroot_credential(
    config: &Config,
    process: &(PathBuf, Vec<String>),
) -> CargoResult<(PathBuf, Vec<String>)> {
    let cred_name = process.0.to_str().unwrap().strip_prefix("cargo:").unwrap();
    let cargo = config.cargo_exe()?;
    let root = cargo
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| format_err!("expected cargo path {}", cargo.display()))?;
    let exe = root.join("libexec").join(format!(
        "cargo-credential-{}{}",
        cred_name,
        std::env::consts::EXE_SUFFIX
    ));
    let mut args = process.1.clone();
    if !args.iter().any(|arg| arg == "{action}") {
        args.push("{action}".to_string());
    }
    Ok((exe, args))
}
