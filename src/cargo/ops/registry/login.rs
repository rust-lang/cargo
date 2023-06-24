//! Interacts with the registry [login API][1].
//!
//! This doesn't really call any web API at this moment. Instead, it's just an
//! operation for `cargo login`.
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#login

use std::io;
use std::io::BufRead;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context as _;
use pasetors::keys::AsymmetricKeyPair;
use pasetors::keys::Generate as _;
use pasetors::paserk::FormatAsPaserk;

use crate::drop_println;
use crate::ops::RegistryCredentialConfig;
use crate::sources::CRATES_IO_DOMAIN;
use crate::util::auth;
use crate::util::auth::paserk_public_from_paserk_secret;
use crate::util::auth::AuthorizationError;
use crate::util::auth::Secret;
use crate::CargoResult;
use crate::Config;

use super::get_source_id;

pub fn registry_login(
    config: &Config,
    token: Option<Secret<&str>>,
    reg: Option<&str>,
    generate_keypair: bool,
    secret_key_required: bool,
    key_subject: Option<&str>,
) -> CargoResult<()> {
    let source_ids = get_source_id(config, None, reg)?;
    let reg_cfg = auth::registry_credential_config(config, &source_ids.original)?;

    let login_url = match super::registry(config, token.clone(), None, reg, false, None) {
        Ok((registry, _)) => Some(format!("{}/me", registry.host())),
        Err(e) if e.is::<AuthorizationError>() => e
            .downcast::<AuthorizationError>()
            .unwrap()
            .login_url
            .map(|u| u.to_string()),
        Err(e) => return Err(e),
    };
    let new_token;
    if generate_keypair || secret_key_required || key_subject.is_some() {
        if !config.cli_unstable().registry_auth {
            let flag = if generate_keypair {
                "generate-keypair"
            } else if secret_key_required {
                "secret-key"
            } else if key_subject.is_some() {
                "key-subject"
            } else {
                unreachable!("how did we get here");
            };
            bail!(
                "the `{flag}` flag is unstable, pass `-Z registry-auth` to enable it\n\
                 See https://github.com/rust-lang/cargo/issues/10519 for more \
                 information about the `{flag}` flag."
            );
        }
        assert!(token.is_none());
        // we are dealing with asymmetric tokens
        let (old_secret_key, old_key_subject) = match &reg_cfg {
            RegistryCredentialConfig::AsymmetricKey((old_secret_key, old_key_subject)) => {
                (Some(old_secret_key), old_key_subject.clone())
            }
            _ => (None, None),
        };
        let secret_key: Secret<String>;
        if generate_keypair {
            assert!(!secret_key_required);
            let kp = AsymmetricKeyPair::<pasetors::version3::V3>::generate().unwrap();
            secret_key = Secret::default().map(|mut key| {
                FormatAsPaserk::fmt(&kp.secret, &mut key).unwrap();
                key
            });
        } else if secret_key_required {
            assert!(!generate_keypair);
            drop_println!(config, "please paste the API secret key below");
            secret_key = Secret::default()
                .map(|mut line| {
                    let input = io::stdin();
                    input
                        .lock()
                        .read_line(&mut line)
                        .with_context(|| "failed to read stdin")
                        .map(|_| line.trim().to_string())
                })
                .transpose()?;
        } else {
            secret_key = old_secret_key
                .cloned()
                .ok_or_else(|| anyhow!("need a secret_key to set a key_subject"))?;
        }
        if let Some(p) = paserk_public_from_paserk_secret(secret_key.as_deref()) {
            drop_println!(config, "{}", &p);
        } else {
            bail!("not a validly formatted PASERK secret key");
        }
        new_token = RegistryCredentialConfig::AsymmetricKey((
            secret_key,
            match key_subject {
                Some(key_subject) => Some(key_subject.to_string()),
                None => old_key_subject,
            },
        ));
    } else {
        new_token = RegistryCredentialConfig::Token(match token {
            Some(token) => token.owned(),
            None => {
                if let Some(login_url) = login_url {
                    drop_println!(
                        config,
                        "please paste the token found on {} below",
                        login_url
                    )
                } else {
                    drop_println!(
                        config,
                        "please paste the token for {} below",
                        source_ids.original.display_registry_name()
                    )
                }

                let mut line = String::new();
                let input = io::stdin();
                input
                    .lock()
                    .read_line(&mut line)
                    .with_context(|| "failed to read stdin")?;
                // Automatically remove `cargo login` from an inputted token to
                // allow direct pastes from `registry.host()`/me.
                Secret::from(line.replace("cargo login", "").trim().to_string())
            }
        });

        if let Some(tok) = new_token.as_token() {
            crates_io::check_token(tok.as_ref().expose())?;
        }
    }
    if &reg_cfg == &new_token {
        config.shell().status("Login", "already logged in")?;
        return Ok(());
    }

    auth::login(config, &source_ids.original, new_token)?;

    config.shell().status(
        "Login",
        format!("token for `{}` saved", reg.unwrap_or(CRATES_IO_DOMAIN)),
    )?;
    Ok(())
}
