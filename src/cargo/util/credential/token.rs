//! Credential provider that uses plaintext tokens in Cargo's config.

use anyhow::Context;
use cargo_credential::{Action, CacheControl, Credential, CredentialResponse, Error, RegistryInfo};
use url::Url;

use crate::{
    core::SourceId,
    ops::RegistryCredentialConfig,
    util::{auth::registry_credential_config_raw, config},
    Config,
};

pub struct TokenCredential<'a> {
    config: &'a Config,
}

impl<'a> TokenCredential<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }
}

impl<'a> Credential for TokenCredential<'a> {
    fn perform(
        &self,
        registry: &RegistryInfo<'_>,
        action: &Action<'_>,
        _args: &[&str],
    ) -> Result<CredentialResponse, Error> {
        let index_url = Url::parse(registry.index_url).context("parsing index url")?;
        let sid = if let Some(name) = registry.name {
            SourceId::for_alt_registry(&index_url, name)
        } else {
            SourceId::for_registry(&index_url)
        }?;
        let previous_token =
            registry_credential_config_raw(self.config, &sid)?.and_then(|c| c.token);

        match action {
            Action::Get(_) => {
                let token = previous_token.ok_or_else(|| Error::NotFound)?.val;
                Ok(CredentialResponse::Get {
                    token,
                    cache: CacheControl::Session,
                    operation_independent: true,
                })
            }
            Action::Login(options) => {
                // Automatically remove `cargo login` from an inputted token to
                // allow direct pastes from `registry.host()`/me.
                let new_token = cargo_credential::read_token(options, registry)?
                    .map(|line| line.replace("cargo login", "").trim().to_string());

                crates_io::check_token(new_token.as_ref().expose()).map_err(Box::new)?;
                config::save_credentials(
                    self.config,
                    Some(RegistryCredentialConfig::Token(new_token)),
                    &sid,
                )?;
                let _ = self.config.shell().status(
                    "Login",
                    format!("token for `{}` saved", sid.display_registry_name()),
                );
                Ok(CredentialResponse::Login)
            }
            Action::Logout => {
                if previous_token.is_none() {
                    return Err(Error::NotFound);
                }
                let reg_name = sid.display_registry_name();
                config::save_credentials(self.config, None, &sid)?;
                let _ = self.config.shell().status(
                    "Logout",
                    format!("token for `{reg_name}` has been removed from local storage"),
                );
                let location = if sid.is_crates_io() {
                    "<https://crates.io/me>".to_string()
                } else {
                    // The URL for the source requires network access to load the config.
                    // That could be a fairly heavy operation to perform just to provide a
                    // help message, so for now this just provides some generic text.
                    // Perhaps in the future this could have an API to fetch the config if
                    // it is cached, but avoid network access otherwise?
                    format!("the `{reg_name}` website")
                };
                eprintln!(
                    "note: This does not revoke the token on the registry server.\n    \
                    If you need to revoke the token, visit {location} and follow the instructions there."
                );
                Ok(CredentialResponse::Logout)
            }
            _ => Err(Error::OperationNotSupported),
        }
    }
}
