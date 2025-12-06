# Lints

Note: [Cargo's linting system is unstable](unstable.md#lintscargo) and can only be used on nightly toolchains

## Allowed-by-default

These lints are all set to the 'allow' level by default.
- [`implicit_minimum_version_req`](#implicit_minimum_version_req)

## Warn-by-default

These lints are all set to the 'warn' level by default.
- [`blanket_hint_mostly_unused`](#blanket_hint_mostly_unused)
- [`unknown_lints`](#unknown_lints)

## `blanket_hint_mostly_unused`
Set to `warn` by default

### What it does
Checks if `hint-mostly-unused` being applied to all dependencies.

### Why it is bad
`hint-mostly-unused` indicates that most of a crate's API surface will go
unused by anything depending on it; this hint can speed up the build by
attempting to minimize compilation time for items that aren't used at all.
Misapplication to crates that don't fit that criteria will slow down the build
rather than speeding it up. It should be selectively applied to dependencies
that meet these criteria. Applying it globally is always a misapplication and
will likely slow down the build.

### Example
```toml
[profile.dev.package."*"]
hint-mostly-unused = true
```

Should instead be:
```toml
[profile.dev.package.huge-mostly-unused-dependency]
hint-mostly-unused = true
```


## `implicit_minimum_version_req`
Set to `allow` by default

### What it does

Checks for dependency version requirements
that do not explicitly specify a full `major.minor.patch` version requirement,
such as `serde = "1"` or `serde = "1.0"`.

This lint currently only applies to caret requirements
(the [default requirements](specifying-dependencies.md#default-requirements)).

### Why it is bad

Version requirements without an explicit full version
can be misleading about the actual minimum supported version.
For example,
`serde = "1"` has an implicit minimum bound of `1.0.0`.
If your code actually requires features from `1.0.219`,
the implicit minimum bound of `1.0.0` gives a false impression about compatibility.

Specifying the full version helps with:

- Accurate minimum version documentation
- Better compatibility with `-Z minimal-versions`
- Clearer dependency constraints for consumers

### Drawbacks

Even with a fully specified version,
the minimum bound might still be incorrect if untested.
This lint helps make the minimum version requirement explicit
but doesn't guarantee correctness.

### Example

```toml
[dependencies]
serde = "1"
```

Should be written as a full specific version:

```toml
[dependencies]
serde = "1.0.219"
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


