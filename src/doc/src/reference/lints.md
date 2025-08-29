# Lints

Note: [Cargo's linting system is unstable](unstable.md#lintscargo) and can only be used on nightly toolchains

## Warn-by-default

These lints are all set to the 'warn' level by default.
- [`invalid_spdx_license_expression`](#invalid_spdx_license_expression)
- [`unknown_lints`](#unknown_lints)

## `invalid_spdx_license_expression`
Set to `warn` by default

### What it does

Checks that the `license` field in `Cargo.toml` is a valid SPDX license expression.
See the doc of [the `license` field] for the SPDX specification version Cargo currently supports.

[the `license` field]: manifest.md#the-license-and-license-file-fields

### Why it is bad

Build tools, package registries, and compliance systems may fail to handle
non-SPDX licenses, which can lead to build failures, rejected uploads,
incorrect license reporting, or legal risks.

### Examples

```toml
license = "MIT / Apache-2.0"       # Invalid: uses "/" instead of "OR"
license = "GPL-3.0 with exception" # Invalid: uses lowercase "with" instead of "WITH"
license = "GPL-3.0+"               # Invalid: uses the deprecated "+" operator instead of "GPL-3.0-or-later"
license = "MIT OR (Apache-2.0"     # Invalid: unclosed parenthesis
```

Use instead:

```toml
license = "MIT OR Apache-2.0"
license = "GPL-3.0 WITH exception"
license = "GPL-3.0-or-later"
license = "(MIT OR Apache-2.0) AND GPL-3.0-or-later WITH Classpath-exception-2.0"
```



## `unknown_lints`
Set to `warn` by default

### What it does
Checks for unknown lints in the `[lints.cargo]` table

### Why it is bad
- The lint name could be misspelled, leading to confusion as to why it is
  not working as expected
- The unknown lint could end up causing an error if `cargo` decides to make
  a lint with the same name in the future

### Example
```toml
[lints.cargo]
this-lint-does-not-exist = "warn"
```


