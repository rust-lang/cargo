{{#option "`--lockfile-path` _PATH_"}}
Changes the path of the lockfile from the default (`<workspace_root>/Cargo.lock`) to _PATH_. _PATH_ must end with 
`Cargo.lock` (e.g. `--lockfile-path /tmp/temporary-lockfile/Cargo.lock`). Note that providing 
`--lockfile-path` will ignore existing lockfile at the default path, and instead will 
either use the lockfile from _PATH_, or write a new lockfile into the provided _PATH_ if it doesn't exist. 
This flag can be used to run most commands in read-only directories, writing lockfile into the provided _PATH_.

This option is only available on the [nightly
channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html) and
requires the `-Z unstable-options` flag to enable (see
[#14421](https://github.com/rust-lang/cargo/issues/14421)).
{{/option}}