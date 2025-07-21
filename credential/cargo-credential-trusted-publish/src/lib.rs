//! Cargo registry trusted-publish credential process library.
//!
//! This library provides a credential provider that implements "trusted publishing"
//! for crates.io using OIDC tokens from CI systems like GitHub Actions.

#![allow(clippy::print_stderr)]

use anyhow::{anyhow, Context, Result};
use cargo_credential::{
    Action, CacheControl, Credential, CredentialResponse, Error, Operation, RegistryInfo, Secret,
};
use reqwest::Client;
use serde::Deserialize;
use std::sync::OnceLock;
use std::time::Duration;

const CRATES_IO_OIDC_EXCHANGE: &str = "https://crates.io/api/v1/oidc/github-actions/exchange";
const CRATES_IO_REVOKE: &str = "https://crates.io/api/v1/oidc/github-actions/revoke";
const CRATES_IO_INDEX: &str = "https://github.com/rust-lang/crates.io-index";

/// Global storage for the current token to enable revocation
static CURRENT_TOKEN: OnceLock<tokio::sync::Mutex<Option<String>>> = OnceLock::new();

/// Response from crates.io OIDC token exchange
#[derive(Deserialize)]
struct ExchangeResponse {
    token: String,
}

/// Implementation of trusted-publish credential provider
pub struct TrustedPublishCredential {
    client: Client,
}

impl TrustedPublishCredential {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("cargo-credential-trusted-publish/0.1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Check if this registry is supported (currently only crates.io)
    fn is_supported_registry(&self, registry: &RegistryInfo<'_>) -> bool {
        registry.index_url == CRATES_IO_INDEX
            || registry.index_url.starts_with("https://index.crates.io/")
            || registry.name == Some("crates-io")
    }

    /// Get OIDC token from environment (GitHub Actions)
    fn get_oidc_token(&self) -> Result<String> {
        std::env::var("ACTIONS_ID_TOKEN")
            .with_context(|| {
                "ACTIONS_ID_TOKEN not found. This credential provider requires running in GitHub Actions with id-token: write permissions."
            })
    }

    /// Exchange OIDC token for crates.io API token
    async fn exchange_for_api_token(&self, oidc_token: &str) -> Result<String> {
        let response = self
            .client
            .post(CRATES_IO_OIDC_EXCHANGE)
            .bearer_auth(oidc_token)
            .send()
            .await
            .context("Failed to send OIDC exchange request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error response".to_string());
            return Err(anyhow!(
                "OIDC token exchange failed with status {}: {}",
                status,
                body
            ));
        }

        let exchange_response: ExchangeResponse = response
            .json()
            .await
            .context("Failed to parse exchange response")?;

        Ok(exchange_response.token)
    }

    /// Revoke the current API token
    async fn revoke_token(&self, token: &str) -> Result<()> {
        let response = self
            .client
            .delete(CRATES_IO_REVOKE)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to send token revocation request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error response".to_string());
            eprintln!(
                "Warning: Failed to revoke token (status {}): {}",
                status, body
            );
        }

        Ok(())
    }

    /// Get or create a token for the current request
    async fn get_token(&self) -> Result<Secret<String>> {
        // Check if we already have a cached token
        let token_mutex = CURRENT_TOKEN.get_or_init(|| tokio::sync::Mutex::new(None));
        let mut token_guard = token_mutex.lock().await;

        if let Some(ref existing_token) = *token_guard {
            return Ok(Secret::from(existing_token.clone()));
        }

        // Get OIDC token and exchange it
        let oidc_token = self.get_oidc_token()?;
        let api_token = self.exchange_for_api_token(&oidc_token).await?;

        // Store the token for potential revocation
        *token_guard = Some(api_token.clone());

        Ok(Secret::from(api_token))
    }

    /// Revoke any cached token
    async fn logout(&self) -> Result<()> {
        let token_mutex = CURRENT_TOKEN.get_or_init(|| tokio::sync::Mutex::new(None));
        let mut token_guard = token_mutex.lock().await;

        if let Some(token) = token_guard.take() {
            self.revoke_token(&token).await?;
        }

        Ok(())
    }
}

impl Default for TrustedPublishCredential {
    fn default() -> Self {
        Self::new()
    }
}

impl Credential for TrustedPublishCredential {
    fn perform(
        &self,
        registry: &RegistryInfo<'_>,
        action: &Action<'_>,
        _args: &[&str],
    ) -> Result<CredentialResponse, Error> {
        // Only support crates.io for now
        if !self.is_supported_registry(registry) {
            return Err(Error::UrlNotSupported);
        }

        // Use tokio runtime for async operations
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| Error::Other(format!("Failed to create async runtime: {}", e).into()))?;

        match action {
            Action::Get(operation) => {
                // Only provide tokens for publish operations in trusted publishing mode
                match operation {
                    Operation::Publish { .. } => {
                        let token = rt
                            .block_on(self.get_token())
                            .map_err(|e| Error::Other(e.into()))?;

                        // Cache for the session to avoid re-exchange, but only for publish operations

                        Ok(CredentialResponse::Get {
                            token,
                            cache: CacheControl::Session, // Cache for the session to avoid re-exchange
                            operation_independent: false, // Only for publish operations
                        })
                    }
                    _ => {
                        // For non-publish operations, let other credential providers handle it
                        Err(Error::NotFound)
                    }
                }
            }
            Action::Login(_) => {
                // Trusted publishing doesn't use traditional login
                Err(Error::OperationNotSupported)
            }
            Action::Logout => {
                rt.block_on(self.logout())
                    .map_err(|e| Error::Other(e.into()))?;
                Ok(CredentialResponse::Logout)
            }
            Action::Unknown => Err(Error::OperationNotSupported),
            _ => Err(Error::OperationNotSupported),
        }
    }
} 