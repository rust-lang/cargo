# cargo-credential-trusted-publish

A Cargo credential provider that implements "trusted publishing" for crates.io using OIDC tokens.

## Overview

This credential provider enables secure publishing to crates.io without storing long-lived API tokens. Instead, it uses OIDC (OpenID Connect) tokens from CI systems like GitHub Actions to obtain short-lived crates.io API tokens that are automatically revoked after use.

## Features

- **No stored secrets**: Uses OIDC tokens instead of long-lived API tokens
- **Just-in-time token acquisition**: Tokens are obtained only when needed for publishing
- **Automatic token revocation**: Tokens are revoked immediately after use
- **CI-native**: Designed to work seamlessly in GitHub Actions and other CI systems

## Usage

### In GitHub Actions

1. Ensure your workflow has the required permissions:
   ```yaml
   permissions:
     id-token: write  # Required for OIDC token
     contents: read
   ```

2. Install the credential provider:
   ```yaml
   - name: Install trusted-publish credential provider
     run: cargo install cargo-credential-trusted-publish
   ```

3. Configure Cargo to use the provider:
   ```yaml
   - name: Configure Cargo credentials
     run: |
       mkdir -p ~/.cargo
       cat >> ~/.cargo/config.toml << 'EOF'
       [registry]
       global-credential-providers = [
         "cargo:token",  # fallback to existing methods
         "cargo-credential-trusted-publish"
       ]
       EOF
   ```

4. Publish your crate:
   ```yaml
   - name: Publish to crates.io
     run: cargo publish
   ```

### Local Development

For local development and testing, you can still use traditional API tokens by setting `CARGO_REGISTRY_TOKEN` or using other credential providers. The trusted-publish provider will only activate when running in a CI environment with OIDC tokens available.

## Requirements

- Rust 1.86+
- GitHub Actions environment with `id-token: write` permissions
- crates.io account configured for trusted publishing

## Security

This credential provider:
- Only works with crates.io (rejects other registries)
- Only provides tokens for publish operations
- Requires OIDC tokens to be present in the environment
- Automatically revokes tokens after use
- Uses secure HTTP client with rustls

## Limitations

- Currently only supports GitHub Actions OIDC tokens
- Only works with crates.io registry
- Requires network access to crates.io API endpoints

## Contributing

This crate is part of the Cargo project. Please file issues and pull requests in the main Cargo repository. 