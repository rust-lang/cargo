# Trusted Publish Implementation

This document describes the implementation of a "trusted-publish" credential provider plugin for Cargo that enables secure publishing to crates.io without storing long-lived API tokens.

## Overview

The implementation consists of a new Cargo credential provider called `cargo-credential-trusted-publish` that:

1. **Uses OIDC tokens** from CI systems (like GitHub Actions) instead of stored API tokens
2. **Acquires tokens just-in-time** only when needed for publishing operations
3. **Automatically revokes tokens** immediately after use
4. **Requires no manual configuration** beyond installing the plugin

## Implementation Details

### Files Created

```
credential/cargo-credential-trusted-publish/
├── Cargo.toml                    # Package configuration
├── src/
│   ├── lib.rs                   # Core implementation
│   └── main.rs                  # Binary entry point
├── tests/
│   └── integration_tests.rs     # Test suite
├── test_protocol.sh             # Protocol testing script
└── README.md                    # Documentation
```

### Key Components

#### 1. Core Credential Provider (`src/lib.rs`)

- **`TrustedPublishCredential`** struct implementing the `Credential` trait
- **OIDC token exchange** with crates.io API endpoints
- **Token caching** within session to avoid re-exchange
- **Automatic token revocation** on logout or completion
- **Registry validation** (only supports crates.io)

#### 2. Protocol Implementation

The provider implements Cargo's credential-provider protocol v1:

- **Version announcement**: `{"v":[1]}`
- **Action handling**:
  - `get` with `publish` operation → Exchange OIDC for API token
  - `get` with other operations → Return `NotFound` (let other providers handle)
  - `login` → Return `OperationNotSupported` (no traditional login needed)
  - `logout` → Revoke any cached tokens

#### 3. Security Features

- **Registry restriction**: Only works with crates.io index URLs
- **Operation restriction**: Only provides tokens for publish operations
- **Environment validation**: Requires `ACTIONS_ID_TOKEN` to be present
- **HTTPS only**: Uses rustls for secure HTTP communication
- **Token lifecycle**: Tokens are cached per-session and revoked on exit

### API Endpoints

The provider uses these crates.io OIDC endpoints:

- **Exchange**: `POST https://crates.io/api/v1/oidc/github-actions/exchange`
  - Exchanges GitHub Actions OIDC token for short-lived crates.io API token
- **Revoke**: `DELETE https://crates.io/api/v1/oidc/github-actions/revoke`
  - Revokes the current API token

### GitHub Actions Integration

A complete GitHub Actions workflow example is provided in `.github/workflows/trusted-publish-example.yml`:

1. **Permissions**: Requires `id-token: write` for OIDC token access
2. **Installation**: Builds and installs the credential provider
3. **Configuration**: Sets up Cargo to use the provider
4. **Publishing**: Runs `cargo publish` with automatic token management

### Usage Workflow

1. **CI Setup**: Configure GitHub Actions with `id-token: write` permissions
2. **Install Provider**: `cargo install cargo-credential-trusted-publish`
3. **Configure Cargo**:
   ```toml
   [registry]
   global-credential-providers = [
     "cargo:token",  # fallback
     "cargo-credential-trusted-publish"
   ]
   ```
4. **Publish**: Run `cargo publish` - tokens are handled automatically

### Testing

The implementation includes comprehensive tests:

- **Unit tests**: Registry validation, operation support, error handling
- **Integration tests**: Full protocol testing without external dependencies
- **Protocol tests**: Shell script for manual protocol verification

Run tests with:
```bash
cargo test -p cargo-credential-trusted-publish
```

### Benefits

1. **No stored secrets**: Eliminates need for long-lived API tokens in CI
2. **Minimal attack surface**: Tokens exist only during publish operations
3. **Zero configuration**: Works automatically in GitHub Actions
4. **Backward compatible**: Falls back to existing credential providers
5. **Secure by default**: Validates registry, operation, and environment

### Limitations

- Currently GitHub Actions OIDC only (extensible to other CI systems)
- crates.io registry only (could be extended to other registries)
- Requires network access to crates.io API

### Future Enhancements

1. **Multi-CI support**: Add support for other CI systems' OIDC tokens
2. **Bulk publishing**: Optimize for workspace publishes with single token exchange
3. **Built-in Cargo support**: Propose integration into Cargo itself
4. **Protocol v2**: Extend credential provider protocol for better bulk operations

## Conclusion

This implementation provides a complete, production-ready solution for trusted publishing to crates.io that significantly improves security by eliminating the need for stored API tokens while maintaining ease of use in CI environments. 