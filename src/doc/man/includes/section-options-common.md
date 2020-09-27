### Common Options

{{#options}}

{{#option "`+`_toolchain_"}}
If Cargo has been installed with rustup, and the first argument to `cargo`
begins with `+`, it will be interpreted as a rustup toolchain name (such
as `+stable` or `+nightly`).
See the [rustup documentation](https://rust-lang.github.io/rustup/overrides.html)
for more information about how toolchain overrides work.
{{/option}}

{{#option "`-h`" "`--help`"}}
Prints help information.
{{/option}}

{{#option "`-Z` _flag_"}}
Unstable (nightly-only) flags to Cargo. Run `cargo -Z help` for details.
{{/option}}

{{/options}}
