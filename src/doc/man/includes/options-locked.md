{{#option "`--locked`"}}
Requires the `Cargo.lock` file be up-to-date. If the lock file is missing,
or it needs to be updated due to changes in the `Cargo.toml` file, for example
a new dependency is added, Cargo will exit with an error.

It may be used in environments where you want to assert that the `Cargo.lock`
file is up-to-date (such as a CI build).
{{/option}}

{{#option "`--offline`"}}
Prevents Cargo from accessing the network for any reason. Without this
flag, Cargo will stop with an error if it needs to access the network and
the network is not available. With this flag, Cargo will attempt to
proceed without the network if possible.

Beware that this may result in different dependency resolution than online
mode. Cargo will restrict itself to crates that are downloaded locally, even
if there might be a newer version as indicated in the local copy of the index.
{{#if (ne command "fetch")}}
See the {{man "cargo-fetch" 1}} command to download dependencies before going
offline.
{{/if}}

May also be specified with the `net.offline` [config value](../reference/config.html).
{{/option}}

{{#option "`--frozen`"}}
Equivalent to specifying both `--locked` and `--offline`.
{{/option}}
