{{#option "`--locked`"}}
Asserts that the exact same dependencies and versions are used as when the
existing `Cargo.lock` file was originally generated. Cargo will exit with an
error when either of the following scenarios arises:

* The lock file is missing.
* Cargo attempted to change the lock file due to a different dependency resolution.

It may be used in environments where deterministic builds are desired,
such as in CI pipelines.
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
