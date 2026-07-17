# cargo-version(1)

## NAME

cargo-version --- Show version information

## SYNOPSIS

`cargo version` [_options_]

## DESCRIPTION

Displays the version of Cargo.

## OPTIONS

{{#options}}

{{#option "`-v`" "`--verbose`" }}
Display additional version information.
{{/option}}

{{/options}}

## EXAMPLES

1. Display the version:

       cargo version

2. The version is also available via flags:

       cargo --version
       cargo -V

3. Display extra version information:

       cargo -Vv

## SEE ALSO
{{man "cargo" 1}}
