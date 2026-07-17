{{#option "`--target` _triple_"}}
{{actionverb}} for the specified target architecture. {{~#if multitarget }} Flag may be specified multiple times. {{~/if}}
{{~#if target-default-to-all-arch}} The default is all architectures.
{{~else}} The default is the host architecture.
{{~/if}} The general format of the triple is
`<arch><sub>-<vendor>-<sys>-<abi>`.

Possible values:
- Any supported target in `rustc --print target-list`.
- `"host-tuple"`, which will internally be substituted by the host's target. This can be particularly useful if you're cross-compiling some crates, and don't want to specify your host's machine as a target (for instance, an `xtask` in a shared project that may be worked on by many hosts).
- A path to a custom target specification. See [Custom Target Lookup Path](../../rustc/targets/custom.html#custom-target-lookup-path) for more information.


This may also be specified with the `build.target` [config value](../reference/config.html).

Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
[build cache](../reference/build-cache.html) documentation for more details.
{{/option}}
