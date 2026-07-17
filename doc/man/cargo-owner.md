# cargo-owner(1)

## NAME

cargo-owner --- Manage the owners of a crate on the registry

## SYNOPSIS

`cargo owner` [_options_] `--add` _login_ [_crate_]\
`cargo owner` [_options_] `--remove` _login_ [_crate_]\
`cargo owner` [_options_] `--list` [_crate_]

## DESCRIPTION

This command will modify the owners for a crate on the registry. Owners of a
crate can upload new versions and yank old versions. Non-team owners can also
modify the set of owners, so take care!

This command requires you to be authenticated with either the `--token` option
or using {{man "cargo-login" 1}}.

If the crate name is not specified, it will use the package name from the
current directory.

See [the reference](../reference/publishing.html#cargo-owner) for more
information about owners and publishing.

## OPTIONS

### Owner Options

{{#options}}

{{#option "`-a`" "`--add` _login_..." }}
Invite the given user or team as an owner.
{{/option}}

{{#option "`-r`" "`--remove` _login_..." }}
Remove the given user or team as an owner.
{{/option}}

{{#option "`-l`" "`--list`" }}
List owners of a crate.
{{/option}}

{{> options-token }}

{{> options-index }}

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

1. List owners of a package:

       cargo owner --list foo

2. Invite an owner to a package:

       cargo owner --add username foo

3. Remove an owner from a package:

       cargo owner --remove username foo

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-login" 1}}, {{man "cargo-publish" 1}}
