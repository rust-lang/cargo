{{#option "`--target` _triple_"}}
{{actionverb}} for the given architecture. The default is the host
architecture. The general format of the triple is
`<arch><sub>-<vendor>-<sys>-<abi>`. Run `rustc --print target-list` for a
list of supported targets.

This may also be specified with the `build.target`
[config value](../reference/config.html).

Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
[build cache](../guide/build-cache.html) documentation for more details.
{{/option}}
