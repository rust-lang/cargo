{{#option "`--profile` _name_" }}
Changes {{lower actionverb}} behavior. Currently only `test` is supported,
which will {{lower actionverb}} with the `#[cfg(test)]` attribute enabled.
This is useful to have it {{lower actionverb}} unit tests which are usually
excluded via the `cfg` attribute. This does not change the actual profile
used.
{{/option}}
