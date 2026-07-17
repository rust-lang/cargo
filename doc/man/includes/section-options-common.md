### Common Options

{{#options}}

{{#option "`+`_toolchain_"}}
If Cargo has been installed with rustup, and the first argument to `cargo`
begins with `+`, it will be interpreted as a rustup toolchain name (such
as `+stable` or `+nightly`).
See the [rustup documentation](https://rust-lang.github.io/rustup/overrides.html)
for more information about how toolchain overrides work.
{{/option}}

{{#option "`--config` _KEY=VALUE_ or _PATH_"}}
Overrides a Cargo configuration value. The argument should be in TOML syntax of `KEY=VALUE`,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the [command-line overrides section](../reference/config.html#command-line-overrides) for more information.
{{/option}}

{{#option "`-C` _PATH_"}}
Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (`Cargo.toml`), as well as
the directories searched for discovering `.cargo/config.toml`, for example. This option must
appear before the command name, for example `cargo -C path/to/my-project build`.

This option is only available on the [nightly
channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html) and
requires the `-Z unstable-options` flag to enable (see
[#10098](https://github.com/rust-lang/cargo/issues/10098)).
{{/option}}

{{#option "`-h`" "`--help`"}}
Prints help information.
{{/option}}

{{#option "`-Z` _flag_"}}
Unstable (nightly-only) flags to Cargo. Run `cargo -Z help` for details.
{{/option}}

{{/options}}
