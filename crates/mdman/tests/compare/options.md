# my-command(1)

## NAME

my-command - A brief description

## SYNOPSIS

`my-command` [`--abc` | `--xyz`] _name_\
`my-command` [`-f` _file_]\
`my-command` (`-m` | `-M`) [_oldbranch_] _newbranch_\
`my-command` (`-d` | `-D`) [`-r`] _branchname_...

## DESCRIPTION

A description of the command.

* One
    * Sub one
    * Sub two
* Two
* Three


## OPTIONS

### Command options

{{#options}}

{{#option "`--foo-bar`"}}
Demo *emphasis*, **strong**, ~~strike~~
{{/option}}

{{#option "`-p` _spec_" "`--package` _spec_"}}
This has multiple flags.
{{/option}}

{{#option "_named-arg..._"}}
A named argument.
{{/option}}

{{/options}}

### Common Options

{{> options-common}}

## EXAMPLES

1. An example

   ```
   my-command --abc
   ```

1. Another example

       my-command --xyz

## SEE ALSO
{{man "other-command" 1}} {{man "abc" 7}}
