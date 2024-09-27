# Lints

Note: [Cargo's linting system is unstable](unstable.md#lintscargo) and can only be used on nightly toolchains

## Allowed-by-default

These lints are all set to the 'allow' level by default.
- [`unexpected_cfgs`](#unexpected_cfgs)

## Warn-by-default

These lints are all set to the 'warn' level by default.
- [`unknown_lints`](#unknown_lints)

## `unexpected_cfgs`
Set to `allow` by default

### What it does
Checks for unexpected cfgs in `[target.'cfg(...)']`

### Why it is bad
The lint helps with verifying that the crate is correctly handling conditional
compilation for different target platforms. It ensures that the cfg settings are
consistent between what is intended and what is used, helping to
catch potential bugs or errors early in the development process.

### Example
```toml
[lints.cargo]
unexpected_cfgs = "warn"
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


