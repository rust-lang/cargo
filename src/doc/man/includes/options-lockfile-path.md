{{#option "`--lockfile-path` _PATH_"}}
Changes the path of the lockfile from the default (`<workspace_root>/Cargo.lock`) to _PATH_. _PATH_ must end with 
`Cargo.lock` (e.g. `--lockfile-path /tmp/temporary-lockfile/Cargo.lock`). Note that providing 
`--lockfile-path` will ignore existing default lockfile (`<workspace_root>/Cargo.lock`), if exists, and instead will 
either use _PATH_ lockfile (or write a new lockfile into the provided path if it doesn't exist). 
This flag can be used to run most commands in read-only directories, writing lockfile into the provided _PATH_.

This option is only available on the [nightly
channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html) and
requires the `-Z unstable-options` flag to enable (see
[#5707](https://github.com/rust-lang/cargo/issues/5707)).
{{/option}}