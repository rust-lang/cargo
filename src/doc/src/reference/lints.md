# Lints

Note: [Cargo's linting system is unstable](unstable.md#lintscargo) and can only be used on nightly toolchains

## Allowed-by-default

These lints are all set to the 'allow' level by default.
- [`implicit_features`](#implicit_features)

## Warn-by-default

These lints are all set to the 'warn' level by default.
- [`unknown_lints`](#unknown_lints)
- [`unused_optional_dependency`](#unused_optional_dependency)

## `implicit_features`
Set to `allow` by default

### What it does
Checks for implicit features for optional dependencies

### Why it is bad
By default, cargo will treat any optional dependency as a [feature]. As of
cargo 1.60, these can be disabled by declaring a feature that activates the
optional dependency as `dep:<name>` (see [RFC #3143]).

In the 2024 edition, `cargo` will stop exposing optional dependencies as
features implicitly, requiring users to add `foo = ["dep:foo"]` if they
still want it exposed.

For more information, see [RFC #3491]

### Example
```toml
edition = "2021"

[dependencies]
bar = { version = "0.1.0", optional = true }

[features]
# No explicit feature activation for `bar`
```

Instead, the dependency should have an explicit feature:
```toml
edition = "2021"

[dependencies]
bar = { version = "0.1.0", optional = true }

[features]
bar = ["dep:bar"]
```

[feature]: https://doc.rust-lang.org/cargo/reference/features.html
[RFC #3143]: https://rust-lang.github.io/rfcs/3143-cargo-weak-namespaced-features.html
[RFC #3491]: https://rust-lang.github.io/rfcs/3491-remove-implicit-features.html


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


## `unused_optional_dependency`
Set to `warn` by default

### What it does
Checks for optional dependencies that are not activated by any feature

### Why it is bad
Starting in the 2024 edition, `cargo` no longer implicitly creates features
for optional dependencies (see [RFC #3491]). This means that any optional
dependency not specified with `"dep:<name>"` in some feature is now unused.
This change may be surprising to users who have been using the implicit
features `cargo` has been creating for optional dependencies.

### Example
```toml
edition = "2024"

[dependencies]
bar = { version = "0.1.0", optional = true }

[features]
# No explicit feature activation for `bar`
```

Instead, the dependency should be removed or activated in a feature:
```toml
edition = "2024"

[dependencies]
bar = { version = "0.1.0", optional = true }

[features]
bar = ["dep:bar"]
```

[RFC #3491]: https://rust-lang.github.io/rfcs/3491-remove-implicit-features.html


