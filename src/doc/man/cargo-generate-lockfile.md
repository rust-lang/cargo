# cargo-generate-lockfile(1)

## NAME

cargo-generate-lockfile --- Generate the lockfile for a package

## SYNOPSIS

`cargo generate-lockfile` [_options_]

## DESCRIPTION

This command will create the `Cargo.lock` lockfile for the current package or
workspace. If the lockfile already exists, it will be rebuilt with the latest
available version of every package.

See also {{man "cargo-update" 1}} which is also capable of creating a `Cargo.lock`
lockfile and has more options for controlling update behavior.

## OPTIONS

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

### Manifest Options

{{#options}}
{{> options-manifest-path }}

{{> options-ignore-rust-version }}

{{#option "`--publish-time` _yyyy-mm-ddThh:mm:ssZ_" }}
Latest publish time allowed for registry packages (Unstable)

This is a best-effort filter on allowed packages, including:
- packages from unsupported registries are always accepted
- only the current yank state is respected, not the state as of `--publish-time`
- precision of the publish time
{{/option}}

{{> options-locked }}

{{> options-lockfile-path }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Create or update the lockfile for the current package or workspace:

       cargo generate-lockfile

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-update" 1}}
