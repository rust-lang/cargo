# Lints

Note: [Cargo's linting system is unstable](unstable.md#lintscargo) and can only be used on nightly toolchains

## Warn-by-default

These lints are all set to the 'warn' level by default.
- [`invalid_spdx_license_expression`](#invalid_spdx_license_expression)
- [`unknown_lints`](#unknown_lints)

## `invalid_spdx_license_expression`
Set to `warn` by default



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


