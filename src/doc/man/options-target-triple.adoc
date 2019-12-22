*--target* _TRIPLE_::
    {actionverb} for the given architecture. The default is the host
    architecture. The general format of the triple is
    `<arch><sub>-<vendor>-<sys>-<abi>`. Run `rustc --print target-list` for a
    list of supported targets.
+
This may also be specified with the `build.target`
linkcargo:reference/config.html[config value].
+
Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
linkcargo:guide/build-cache.html[build cache] documentation for more details.
