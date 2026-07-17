{{#option "`--keep-going`"}}
Build as many crates in the dependency graph as possible, rather than aborting
the build on the first one that fails to build.

For example if the current package depends on dependencies `fails` and `works`,
one of which fails to build, `cargo {{command}} -j1` may or may not build the
one that succeeds (depending on which one of the two builds Cargo picked to run
first), whereas `cargo {{command}} -j1 --keep-going` would definitely run both
builds, even if the one run first fails.
{{/option}}
