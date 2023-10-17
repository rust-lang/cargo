{{#option "`--target-dir` _directory_"}}
Directory for all generated artifacts and intermediate files. May also be
specified with the `CARGO_TARGET_DIR` environment variable, or the
`build.target-dir` [config value](../reference/config.html).
{{#if temp-target-dir}} Defaults to a new temporary folder located in the
temporary directory of the platform. 

When using `--path`, by default it will use `target` directory in the workspace
of the local crate unless `--target-dir`
is specified.
{{else}} Defaults to `target` in the root of the workspace.
{{/if}}
{{/option}}
