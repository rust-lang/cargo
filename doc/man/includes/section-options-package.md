### Package Selection

By default, the package in the current working directory is selected. The `-p`
flag can be used to choose a different package in a workspace.

{{#options}}

{{#option "`-p` _spec_" "`--package` _spec_" }}
The package to {{lower actionverb}}. See {{man "cargo-pkgid" 1}} for the SPEC
format.
{{/option}}

{{/options}}
