# Registry Authentication
Cargo authenticates to registries with credential providers. These
credential providers are external executables or built-in providers that Cargo
uses to store and retrieve credentials.

Using alternative registries with authentication *requires* a credential provider to be configured
to avoid unknowingly storing unencrypted credentials on disk. For historical reasons, public
(non-authenticated) registries do not require credential provider configuration, and the `cargo:token`
provider is used if no providers are configured.

Cargo also includes platform-specific providers that use the operating system to securely store
tokens. The `cargo:token` provider is also included which stores credentials in unencrypted plain
text in the [credentials](config.md#credentials) file. 

## Recommended configuration
It's recommended to configure a global credential provider list in `$CARGO_HOME/config.toml`
which defaults to:
* Windows: `%USERPROFILE%\.cargo\config.toml`
* Unix: `~/.cargo/config.toml`

This recommended configuration uses the operating system provider, with a fallback to `cargo:token`
to look in Cargo's [credentials](config.md#credentials) file or environment variables.

Some private registries may also recommend a registry-specific credential-provider. Check your
registry's documentation to see if this is the case.

### macOS configuration
```toml
# ~/.cargo/config.toml
[registry]
global-credential-providers = ["cargo:token", "cargo:macos-keychain"]
```

### Linux (libsecret) configuration
```toml
# ~/.cargo/config.toml
[registry]
global-credential-providers = ["cargo:token", "cargo:libsecret"]
```

### Windows configuration
```toml
# %USERPROFILE%\.cargo\config.toml
[registry]
global-credential-providers = ["cargo:token", "cargo:wincred"]
```

See [`registry.global-credential-providers`](config.md#registryglobal-credential-providers)
for more details.

## Built-in providers
Cargo includes several built-in credential providers. The available built-in providers
may change in future Cargo releases (though there are currently no plans to do so).

### `cargo:token`
Uses Cargo's [credentials](config.md#credentials) file to store tokens unencrypted in plain text.
When retrieving tokens, checks the `CARGO_REGISTRIES_<NAME>_TOKEN` environment variable.
If this credential provider is not listed, then the `*_TOKEN` environment variables will not work.

### `cargo:wincred`
Uses the Windows Credential Manager to store tokens.

The credentials are stored as `cargo-registry:<index-url>` in the Credential Manager
under "Windows Credentials".

### `cargo:macos-keychain`
Uses the macOS Keychain to store tokens.

The Keychain Access app can be used to view stored tokens.

### `cargo:libsecret`
Uses [libsecret](https://wiki.gnome.org/Projects/Libsecret) to store tokens.

On GNOME, credentials can be viewed using [GNOME Keyring](https://wiki.gnome.org/Projects/GnomeKeyring)
applications.

### `cargo:token-from-stdout <command> <args>`
Launch a subprocess that returns a token on stdout. Newlines will be trimmed.
* The process inherits the user's stdin and stderr.
* It should exit 0 on success, and nonzero on error.
* [`cargo login`] and [`cargo logout`] are not supported and return an error if used.

The following environment variables will be provided to the executed command:

* `CARGO` --- Path to the `cargo` binary executing the command.
* `CARGO_REGISTRY_INDEX_URL` --- The URL of the registry index.
* `CARGO_REGISTRY_NAME_OPT` --- Optional name of the registry. Should not be used as a lookup key.

Arguments will be passed on to the subcommand.

[`cargo login`]: ../commands/cargo-login.md
[`cargo logout`]: ../commands/cargo-logout.md

## Credential plugins
For credential provider plugins that follow Cargo's [credential provider protocol](credential-provider-protocol.md),
the configuration value should be a string with the path to the executable (or the executable name if on the `PATH`).

For example, to install [cargo-credential-1password](https://crates.io/crates/cargo-credential-1password)
from crates.io do the following:

Install the provider with `cargo install cargo-credential-1password`

In the config, add to (or create) `registry.global-credential-providers`:
```toml
[registry]
global-credential-providers = ["cargo:token", "cargo-credential-1password --email you@example.com"]
```

The values in `global-credential-providers` are split on spaces into path and command-line arguments. To
define a global credential provider where the path or arguments contain spaces, use
the [`[credential-alias]` table](config.md#credential-alias).
