# cargo-login(1)

## NAME

cargo-login - Save an API token from the registry locally

## SYNOPSIS

`cargo login` [_options_] [_token_]

## DESCRIPTION

This command will save the API token to disk so that commands that require
authentication, such as {{man "cargo-publish" 1}}, will be automatically
authenticated. The token is saved in `$CARGO_HOME/credentials.toml`. `CARGO_HOME`
defaults to `.cargo` in your home directory.

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

1. Save the API token to disk:

       cargo login

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-publish" 1}}
