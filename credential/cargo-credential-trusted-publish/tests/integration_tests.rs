use cargo_credential::{Action, Operation, RegistryInfo, Error, Credential};
use cargo_credential_trusted_publish::TrustedPublishCredential;

#[test]
fn test_unsupported_registry() {
    let credential = TrustedPublishCredential::new();
    let registry = RegistryInfo {
        index_url: "https://example.com/registry",
        name: Some("example"),
        headers: vec![],
    };
    let action = Action::Get(Operation::Publish {
        name: "test-crate",
        vers: "0.1.0",
        cksum: "abc123",
    });

    let result = credential.perform(&registry, &action, &[]);
    assert!(matches!(result, Err(Error::UrlNotSupported)));
}

#[test]
fn test_supported_registry_crates_io() {
    let credential = TrustedPublishCredential::new();
    let registry = RegistryInfo {
        index_url: "https://github.com/rust-lang/crates.io-index",
        name: Some("crates-io"),
        headers: vec![],
    };
    let action = Action::Get(Operation::Read);

    // Should return NotFound for non-publish operations, not UrlNotSupported
    let result = credential.perform(&registry, &action, &[]);
    assert!(matches!(result, Err(Error::NotFound)));
}

#[test]
fn test_login_not_supported() {
    let credential = TrustedPublishCredential::new();
    let registry = RegistryInfo {
        index_url: "https://github.com/rust-lang/crates.io-index",
        name: Some("crates-io"),
        headers: vec![],
    };
    let action = Action::Login(cargo_credential::LoginOptions {
        token: None,
        login_url: None,
    });

    let result = credential.perform(&registry, &action, &[]);
    assert!(matches!(result, Err(Error::OperationNotSupported)));
}

#[test]
fn test_logout_without_token() {
    let credential = TrustedPublishCredential::new();
    let registry = RegistryInfo {
        index_url: "https://github.com/rust-lang/crates.io-index",
        name: Some("crates-io"),
        headers: vec![],
    };
    let action = Action::Logout;

    // Should succeed even without a token to revoke
    let result = credential.perform(&registry, &action, &[]);
    assert!(result.is_ok());
}

#[cfg(test)]
mod oidc_tests {
    use super::*;
    use std::env;
    use cargo_credential::Credential;

    #[test]
    fn test_missing_oidc_token() {
        // Ensure ACTIONS_ID_TOKEN is not set
        unsafe { env::remove_var("ACTIONS_ID_TOKEN"); }
        
        let credential = TrustedPublishCredential::new();
        let registry = RegistryInfo {
            index_url: "https://github.com/rust-lang/crates.io-index",
            name: Some("crates-io"),
            headers: vec![],
        };
        let action = Action::Get(Operation::Publish {
            name: "test-crate",
            vers: "0.1.0",
            cksum: "abc123",
        });

        let result = credential.perform(&registry, &action, &[]);
        assert!(result.is_err());
        
        if let Err(Error::Other(e)) = result {
            assert!(e.to_string().contains("ACTIONS_ID_TOKEN"));
        } else {
            panic!("Expected Other error containing ACTIONS_ID_TOKEN message");
        }
    }

    #[test]
    #[ignore] // Only run with a valid OIDC token in CI
    fn test_with_valid_oidc_token() {
        // This test requires a valid ACTIONS_ID_TOKEN environment variable
        // and should only be run in actual GitHub Actions
        if env::var("ACTIONS_ID_TOKEN").is_err() {
            return; // Skip test if no token available
        }

        let credential = TrustedPublishCredential::new();
        let registry = RegistryInfo {
            index_url: "https://github.com/rust-lang/crates.io-index",
            name: Some("crates-io"),
            headers: vec![],
        };
        let action = Action::Get(Operation::Publish {
            name: "test-crate",
            vers: "0.1.0",
            cksum: "abc123",
        });

        // This might fail due to crates.io API restrictions, but should at least
        // attempt the OIDC exchange
        let _result = credential.perform(&registry, &action, &[]);
        // We don't assert success here since it depends on external API
    }
} 