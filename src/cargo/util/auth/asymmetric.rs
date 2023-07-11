//! Registry asymmetric authentication support. See [RFC 3231] for more.
//!
//! [RFC 3231]: https://rust-lang.github.io/rfcs/3231-cargo-asymmetric-tokens.html

use pasetors::keys::AsymmetricPublicKey;
use pasetors::keys::AsymmetricSecretKey;
use pasetors::paserk;
use pasetors::paserk::FormatAsPaserk;
use pasetors::version3;
use pasetors::version3::PublicToken;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::core::SourceId;
use crate::ops::RegistryCredentialConfig;
use crate::CargoResult;

use super::Mutation;
use super::Secret;

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
    /// This field is not yet used. This field can be set to a value >1 to
    /// indicate a breaking change in the token format.
    #[serde(skip_serializing_if = "Option::is_none")]
    v: Option<u8>,
}

/// The footer of an asymmetric token as describe in RFC 3231.
#[derive(serde::Serialize)]
struct Footer<'a> {
    url: &'a str,
    kip: paserk::Id,
}

/// Checks that a secret key is valid, and returns the associated public key in
/// Paserk format.
pub fn paserk_public_from_paserk_secret(secret_key: Secret<&str>) -> Option<String> {
    let secret: Secret<AsymmetricSecretKey<version3::V3>> =
        secret_key.map(|key| key.try_into()).transpose().ok()?;
    let public: AsymmetricPublicKey<version3::V3> = secret
        .as_ref()
        .map(|key| key.try_into())
        .transpose()
        .ok()?
        .expose();
    let mut paserk_pub_key = String::new();
    FormatAsPaserk::fmt(&public, &mut paserk_pub_key).unwrap();
    Some(paserk_pub_key)
}

/// Generates a public token from a registry's `credential` configuration for
/// authenticating to a `source_id`
///
/// An optional `mutation` for authenticating a mutation operation aganist the
/// registry.
pub fn public_token_from_credential(
    credential: RegistryCredentialConfig,
    source_id: &SourceId,
    mutation: Option<&'_ Mutation<'_>>,
) -> CargoResult<Secret<String>> {
    let RegistryCredentialConfig::AsymmetricKey((secret_key, secret_key_subject)) = credential
    else {
        anyhow::bail!("credential must be an asymmetric secret key")
    };

    let secret: Secret<AsymmetricSecretKey<version3::V3>> =
        secret_key.map(|key| key.as_str().try_into()).transpose()?;
    let public: AsymmetricPublicKey<version3::V3> = secret
        .as_ref()
        .map(|key| key.try_into())
        .transpose()?
        .expose();
    let kip = (&public).try_into()?;
    let iat = OffsetDateTime::now_utc();

    let message = Message {
        iat: &iat.format(&Rfc3339)?,
        sub: secret_key_subject.as_deref(),
        mutation: mutation.and_then(|m| {
            Some(match m {
                Mutation::PrePublish => return None,
                Mutation::Publish { .. } => "publish",
                Mutation::Yank { .. } => "yank",
                Mutation::Unyank { .. } => "unyank",
                Mutation::Owners { .. } => "owners",
            })
        }),
        name: mutation.and_then(|m| {
            Some(match m {
                Mutation::PrePublish => return None,
                Mutation::Publish { name, .. }
                | Mutation::Yank { name, .. }
                | Mutation::Unyank { name, .. }
                | Mutation::Owners { name, .. } => *name,
            })
        }),
        vers: mutation.and_then(|m| {
            Some(match m {
                Mutation::PrePublish | Mutation::Owners { .. } => return None,
                Mutation::Publish { vers, .. }
                | Mutation::Yank { vers, .. }
                | Mutation::Unyank { vers, .. } => *vers,
            })
        }),
        cksum: mutation.and_then(|m| {
            Some(match m {
                Mutation::PrePublish
                | Mutation::Yank { .. }
                | Mutation::Unyank { .. }
                | Mutation::Owners { .. } => return None,
                Mutation::Publish { cksum, .. } => *cksum,
            })
        }),
        challenge: None, // todo: PASETO with challenges
        v: None,
    };

    let footer = Footer {
        url: &source_id.url().to_string(),
        kip,
    };

    let secret = secret
        .map(|secret| {
            PublicToken::sign(
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
        .transpose()?;

    Ok(secret)
}
