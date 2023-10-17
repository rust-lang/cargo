# cargo-login(1)

## NAME

cargo-login --- Log in to a registry

## SYNOPSIS

`cargo login` [_options_] [_token_] [`--` _args_]

## DESCRIPTION

This command will run a credential provider to save a token so that commands
that require authentication, such as {{man "cargo-publish" 1}}, will be
automatically authenticated.

All the arguments following the two dashes (`--`) are passed to the credential provider.

For the default `cargo:token` credential provider, the token is saved
in `$CARGO_HOME/credentials.toml`. `CARGO_HOME` defaults to `.cargo`
in your home directory.

If a registry has a credential-provider specified, it will be used. Otherwise,
the providers from the config value `registry.global-credential-providers` will
be attempted, starting from the end of the list.

If the _token_ argument is not specified, it will be read from stdin.

The API token for crates.io may be retrieved from <https://crates.io/me>.

Take care to keep the token secret, it should not be shared with anyone else.

## OPTIONS

### Login Options

{{#options}}
{{> options-registry }}
{{/options}}

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Save the token for the default registry:

       cargo login

2. Save the token for a specific registry:

       cargo login --registry my-registry

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-logout" 1}}, {{man "cargo-publish" 1}}
