# Lints

Note: [Cargo's linting system is unstable](unstable.md#lintscargo) and can only be used on nightly toolchains



| Group                | Description                                                      | Default level |
|----------------------|------------------------------------------------------------------|---------------|
| `cargo::complexity`  | code that does something simple but in a complex way             | warn          |
| `cargo::correctness` | code that is outright wrong or useless                           | deny          |
| `cargo::nursery`     | new lints that are still under development                       | allow         |
| `cargo::pedantic`    | lints which are rather strict or have occasional false positives | allow         |
| `cargo::perf`        | code that can be written to run faster                           | warn          |
| `cargo::restriction` | lints which prevent the use of Cargo features                    | allow         |
| `cargo::style`       | code that should be written in a more idiomatic way              | warn          |
| `cargo::suspicious`  | code that is most likely wrong or useless                        | warn          |


## Allowed-by-default

These lints are all set to the 'allow' level by default.
- [`implicit_minimum_version_req`](#implicit_minimum_version_req)

## Warn-by-default

These lints are all set to the 'warn' level by default.
- [`blanket_hint_mostly_unused`](#blanket_hint_mostly_unused)
- [`non_kebab_case_bin`](#non_kebab_case_bin)
- [`unknown_lints`](#unknown_lints)

## `blanket_hint_mostly_unused`
Group: `suspicious`

Level: `warn`

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
Group: `pedantic`

Level: `allow`

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


## `non_kebab_case_bin`
Group: `style`

Level: `warn`

### What it does

Detect binary names, explicit and implicit, that are not kebab-case

### Why it is bad

Kebab-case binary names is a common convention among command line tools.

### Drawbacks

It would be disruptive to existing users to change the binary name.

A binary may need to conform to externally controlled conventions which can include a different naming convention.

GUI applications may wish to choose a more user focused naming convention, like "Title Case" or "Sentence case".

### Example

```toml
[[bin]]
name = "foo_bar"
```

Should be written as:

```toml
[[bin]]
name = "foo-bar"
```


## `unknown_lints`
Group: `suspicious`

Level: `warn`

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


