{{#option "`--target` _triple_"}}
{{actionverb}} for the specified target {{~#if multitarget }} (may be specified multiple times) {{~/if}}

{{~#if target-default-to-all-arch}} The default is all architectures.
{{~else}} The default is the host architecture.
{{~/if}} The general format of the triple is
`<arch><sub>-<vendor>-<sys>-<abi>`.

You may specify the following kinds of targets:
- Any supported target in `rustc --print target-list` (note: you have to install/add the target to use it).
- `host`, which will internally be substituted by the host's target. This can be particularly useful if you're cross-compiling some crates, and don't want to specify your host's machine as a target (for instance, an `xtask` in a shared project that may be worked on by many hosts).
- A path to a custom target specification (further reading [here](https://doc.rust-lang.org/rustc/targets/custom.html#custom-target-lookup-path)).


This may also be specified with the `build.target` [config value](../reference/config.html).

**Note**: Specifying this flag makes Cargo run in a different mode where the target artifacts are placed in a separate directory. See the [build cache](../reference/build-cache.html) documentation for more details.
{{/option}}
