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
- [`non_kebab_case_features`](#non_kebab_case_features)
- [`non_kebab_case_packages`](#non_kebab_case_packages)
- [`non_snake_case_features`](#non_snake_case_features)
- [`non_snake_case_packages`](#non_snake_case_packages)

## Warn-by-default

These lints are all set to the 'warn' level by default.
- [`blanket_hint_mostly_unused`](#blanket_hint_mostly_unused)
- [`non_kebab_case_bins`](#non_kebab_case_bins)
- [`redundant_homepage`](#redundant_homepage)
- [`redundant_readme`](#redundant_readme)
- [`unknown_lints`](#unknown_lints)
- [`unused_workspace_dependencies`](#unused_workspace_dependencies)

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


## `non_kebab_case_bins`
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


## `non_kebab_case_features`
Group: `restriction`

Level: `allow`

### What it does

Detect feature names that are not kebab-case.

### Why it is bad

Having multiple naming styles within a workspace can be confusing.

### Drawbacks

Users would expect that a feature tightly coupled to a dependency would match the dependency's name.

### Example

```toml
[features]
foo_bar = []
```

Should be written as:

```toml
[features]
foo-bar = []
```


## `non_kebab_case_packages`
Group: `restriction`

Level: `allow`

### What it does

Detect package names that are not kebab-case.

### Why it is bad

Having multiple naming styles within a workspace can be confusing.

### Drawbacks

Users have to mentally translate package names to namespaces in Rust.

### Example

```toml
[package]
name = "foo_bar"
```

Should be written as:

```toml
[package]
name = "foo-bar"
```


## `non_snake_case_features`
Group: `restriction`

Level: `allow`

### What it does

Detect feature names that are not snake-case.

### Why it is bad

Having multiple naming styles within a workspace can be confusing.

### Drawbacks

Users would expect that a feature tightly coupled to a dependency would match the dependency's name.

### Example

```toml
[features]
foo-bar = []
```

Should be written as:

```toml
[features]
foo_bar = []
```


## `non_snake_case_packages`
Group: `restriction`

Level: `allow`

### What it does

Detect package names that are not snake-case.

### Why it is bad

Having multiple naming styles within a workspace can be confusing.

### Drawbacks

Users have to mentally translate package names to namespaces in Rust.

### Example

```toml
[package]
name = "foo_bar"
```

Should be written as:

```toml
[package]
name = "foo-bar"
```


## `redundant_homepage`
Group: `style`

Level: `warn`

### What it does

Checks if the value of `package.homepage` is already covered by another field.

See also [`package.homepage` reference documentation](manifest.md#the-homepage-field).

### Why it is bad

When package browsers render each link, a redundant link adds visual noise.

### Drawbacks

### Example

```toml
[package]
name = "foo"
homepage = "https://github.com/rust-lang/cargo/"
repository = "https://github.com/rust-lang/cargo/"
```

Should be written as:

```toml
[package]
name = "foo"
repository = "https://github.com/rust-lang/cargo/"
```


## `redundant_readme`
Group: `style`

Level: `warn`

### What it does

Checks for `package.readme` fields that can be inferred.

See also [`package.readme` reference documentation](manifest.md#the-readme-field).

### Why it is bad

Adds boilerplate.

### Drawbacks

It might not be obvious if they named their file correctly.

### Example

```toml
[package]
name = "foo"
readme = "README.md"
```

Should be written as:

```toml
[package]
name = "foo"
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


## `unused_workspace_dependencies`
Group: `suspicious`

Level: `warn`

### What it does
Checks for any entry in `[workspace.dependencies]` that has not been inherited

### Why it is bad
They can give the false impression that these dependencies are used

### Example
```toml
[workspace.dependencies]
regex = "1"

[dependencies]
```


