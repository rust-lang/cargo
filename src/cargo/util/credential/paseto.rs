//! Credential provider that implements PASETO asymmetric tokens stored in Cargo's config.

use anyhow::Context;
use cargo_credential::{
    Action, CacheControl, Credential, CredentialResponse, Error, Operation, RegistryInfo, Secret,
};
use clap::Command;
use pasetors::{
    keys::{AsymmetricKeyPair, AsymmetricPublicKey, AsymmetricSecretKey, Generate},
    paserk::FormatAsPaserk,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use url::Url;

use crate::{
    core::SourceId,
    ops::RegistryCredentialConfig,
    util::{auth::registry_credential_config_raw, command_prelude::opt, config},
    Config,
};

/// The main body of an asymmetric token as describe in RFC 3231.
#[derive(serde::Serialize)]
struct Message<'a> {
    iat: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mutation: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vers: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cksum: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    challenge: Option<&'a str>,
    /// This field is not yet used. This field can be set to a value >1 to indicate a breaking change in the token format.
    #[serde(skip_serializing_if = "Option::is_none")]
    v: Option<u8>,
}
/// The footer of an asymmetric token as describe in RFC 3231.
#[derive(serde::Serialize)]
struct Footer<'a> {
    url: &'a str,
    kip: pasetors::paserk::Id,
}

pub(crate) struct PasetoCredential<'a> {
    config: &'a Config,
}

impl<'a> PasetoCredential<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }
}

impl<'a> Credential for PasetoCredential<'a> {
    fn perform(
        &self,
        registry: &RegistryInfo<'_>,
        action: &Action<'_>,
        args: &[&str],
    ) -> Result<CredentialResponse, Error> {
        let index_url = Url::parse(registry.index_url).context("parsing index url")?;
        let sid = if let Some(name) = registry.name {
            SourceId::for_alt_registry(&index_url, name)
        } else {
            SourceId::for_registry(&index_url)
        }?;

        let reg_cfg = registry_credential_config_raw(self.config, &sid)?;

        let matches = Command::new("cargo:paseto")
            .no_binary_name(true)
            .arg(opt("key-subject", "Set the key subject for this registry").value_name("SUBJECT"))
            .try_get_matches_from(args)
            .map_err(Box::new)?;
        let key_subject = matches.get_one("key-subject").map(String::as_str);

        match action {
            Action::Get(operation) => {
                let Some(reg_cfg) = reg_cfg else {
                    return Err(Error::NotFound);
                };
                let Some(secret_key) = reg_cfg.secret_key.as_ref() else {
                    return Err(Error::NotFound);
                };

                let secret_key_subject = reg_cfg.secret_key_subject;
                let secret: Secret<AsymmetricSecretKey<pasetors::version3::V3>> = secret_key
                    .val
                    .as_ref()
                    .map(|key| key.as_str().try_into())
                    .transpose()
                    .context("failed to load private key")?;
                let public: AsymmetricPublicKey<pasetors::version3::V3> = secret
                    .as_ref()
                    .map(|key| key.try_into())
                    .transpose()
                    .context("failed to load public key from private key")?
                    .expose();
                let kip: pasetors::paserk::Id = (&public).into();

                let iat = OffsetDateTime::now_utc();

                let message = Message {
                    iat: &iat.format(&Rfc3339).unwrap(),
                    sub: secret_key_subject.as_deref(),
                    mutation: match operation {
                        Operation::Publish { .. } => Some("publish"),
                        Operation::Yank { .. } => Some("yank"),
                        Operation::Unyank { .. } => Some("unyank"),
                        Operation::Owners { .. } => Some("owners"),
                        _ => None,
                    },
                    name: match operation {
                        Operation::Publish { name, .. }
                        | Operation::Yank { name, .. }
                        | Operation::Unyank { name, .. }
                        | Operation::Owners { name, .. } => Some(name),
                        _ => None,
                    },
                    vers: match operation {
                        Operation::Publish { vers, .. }
                        | Operation::Yank { vers, .. }
                        | Operation::Unyank { vers, .. } => Some(vers),
                        _ => None,
                    },
                    cksum: match operation {
                        Operation::Publish { cksum, .. } => Some(cksum),
                        _ => None,
                    },
                    challenge: None, // todo: PASETO with challenges
                    v: None,
                };
                let footer = Footer {
                    url: &registry.index_url,
                    kip,
                };

                // Only read operations can be cached with asymmetric tokens.
                let cache = match operation {
                    Operation::Read => CacheControl::Session,
                    _ => CacheControl::Never,
                };

                let token = secret
                    .map(|secret| {
                        pasetors::version3::PublicToken::sign(
                            &secret,
                            serde_json::to_string(&message)
                                .expect("cannot serialize")
                                .as_bytes(),
                            Some(
                                serde_json::to_string(&footer)
                                    .expect("cannot serialize")
                                    .as_bytes(),
                            ),
                            None,
                        )
                    })
                    .transpose()
                    .context("failed to sign request")?;

                Ok(CredentialResponse::Get {
                    token,
                    cache,
                    operation_independent: false,
                })
            }
            Action::Login(options) => {
                let old_key_subject = reg_cfg.and_then(|cfg| cfg.secret_key_subject);
                let new_token;
                let secret_key: Secret<String>;
                if let Some(key) = &options.token {
                    secret_key = key.clone().map(str::to_string);
                } else {
                    let kp = AsymmetricKeyPair::<pasetors::version3::V3>::generate().unwrap();
                    secret_key = Secret::default().map(|mut key| {
                        FormatAsPaserk::fmt(&kp.secret, &mut key).unwrap();
                        key
                    });
                }

                if let Some(p) = paserk_public_from_paserk_secret(secret_key.as_deref()) {
                    eprintln!("{}", &p);
                } else {
                    return Err("not a validly formatted PASERK secret key".into());
                }
                new_token = RegistryCredentialConfig::AsymmetricKey((
                    secret_key,
                    match key_subject {
                        Some(key_subject) => Some(key_subject.to_string()),
                        None => old_key_subject,
                    },
                ));
                config::save_credentials(self.config, Some(new_token), &sid)?;
                Ok(CredentialResponse::Login)
            }
            Action::Logout => {
                if reg_cfg.and_then(|c| c.secret_key).is_some() {
                    config::save_credentials(self.config, None, &sid)?;
                    let reg_name = sid.display_registry_name();
                    let _ = self.config.shell().status(
                        "Logout",
                        format!("secret-key for `{reg_name}` has been removed from local storage"),
                    );
                    Ok(CredentialResponse::Logout)
                } else {
                    Err(Error::NotFound)
                }
            }
            _ => Err(Error::OperationNotSupported),
        }
    }
}

/// Checks that a secret key is valid, and returns the associated public key in Paserk format.
pub(crate) fn paserk_public_from_paserk_secret(secret_key: Secret<&str>) -> Option<String> {
    let secret: Secret<AsymmetricSecretKey<pasetors::version3::V3>> =
        secret_key.map(|key| key.try_into()).transpose().ok()?;
    let public: AsymmetricPublicKey<pasetors::version3::V3> = secret
        .as_ref()
        .map(|key| key.try_into())
        .transpose()
        .ok()?
        .expose();
    let mut paserk_pub_key = String::new();
    FormatAsPaserk::fmt(&public, &mut paserk_pub_key).unwrap();
    Some(paserk_pub_key)
}
