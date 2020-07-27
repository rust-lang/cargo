## Features

Cargo supports features to allow expression of:

* conditional compilation options (usable through `cfg` attributes);
* optional dependencies, which enhance a package, but are not required; and
* clusters of optional dependencies, such as `postgres-all`, that would include the
  `postgres` package, the `postgres-macros` package, and possibly other packages
  (such as development-time mocking libraries, debugging tools, etc.).

A feature of a package is either an optional dependency, or a set of other
features.

### The `[features]` section

Features are defined in the `[features]` table of `Cargo.toml`. The format for
specifying features is:

```toml
[package]
name = "awesome"

[features]
# The default set of optional packages. Most people will want to use these
# packages, but they are strictly optional. Note that `session` is not a package
# but rather another feature listed in this manifest.
default = ["jquery", "uglifier", "session"]

# A feature with no dependencies is used mainly for conditional compilation,
# like `#[cfg(feature = "go-faster")]`.
go-faster = []

# The `secure-password` feature depends on the bcrypt package. This aliasing
# will allow people to talk about the feature in a higher-level way and allow
# this package to add more requirements to the feature in the future.
secure-password = ["bcrypt"]

# Features can be used to reexport features of other packages. The `session`
# feature of package `awesome` will ensure that the `session` feature of the
# package `cookie` is also enabled.
session = ["cookie/session"]

[dependencies]
# These packages are mandatory and form the core of this package’s distribution.
cookie = "1.2.0"
oauth = "1.1.0"
route-recognizer = "=2.1.0"

# A list of all of the optional dependencies, some of which are included in the
# above `features`. They can be opted into by apps.
jquery = { version = "1.0.2", optional = true }
uglifier = { version = "1.5.3", optional = true }
bcrypt = { version = "*", optional = true }
civet = { version = "*", optional = true }
```

To use the package `awesome`:

```toml
[dependencies.awesome]
version = "1.3.5"
default-features = false # do not include the default features, and optionally
                         # cherry-pick individual features
features = ["secure-password", "civet"]
```

### Rules

The usage of features is subject to a few rules:

* Feature names must not conflict with other package names in the manifest. This
  is because they are opted into via `features = [...]`, which only has a single
  namespace.
* With the exception of the `default` feature, all features are opt-in. To opt
  out of the default feature, use `default-features = false` and cherry-pick
  individual features.
* Feature groups are not allowed to cyclically depend on one another.
* Dev-dependencies cannot be optional.
* Features groups can only reference optional dependencies.
* When a feature is selected, Cargo will call `rustc` with `--cfg
  feature="${feature_name}"`. If a feature group is included, it and all of its
  individual features will be included. This can be tested in code via
  `#[cfg(feature = "foo")]`.

Note that it is explicitly allowed for features to not actually activate any
optional dependencies. This allows packages to internally enable/disable
features without requiring a new dependency.

> **Note**: [crates.io] requires feature names to only contain ASCII letters,
> digits, `_`, `-`, or `+`.

### Usage in end products

One major use-case for this feature is specifying optional features in
end-products. For example, the Servo package may want to include optional
features that people can enable or disable when they build it.

In that case, Servo will describe features in its `Cargo.toml` and they can be
enabled using command-line flags:

```console
$ cargo build --release --features "shumway pdf"
```

Default features could be excluded using `--no-default-features`.

### Usage in packages

In most cases, the concept of *optional dependency* in a library is best
expressed as a separate package that the top-level application depends on.

However, high-level packages, like Iron or Piston, may want the ability to
curate a number of packages for easy installation. The current Cargo system
allows them to curate a number of mandatory dependencies into a single package
for easy installation.

In some cases, packages may want to provide additional curation for optional
dependencies:

* grouping a number of low-level optional dependencies together into a single
  high-level feature;
* specifying packages that are recommended (or suggested) to be included by
  users of the package; and
* including a feature (like `secure-password` in the motivating example) that
  will only work if an optional dependency is available, and would be difficult
  to implement as a separate package (for example, it may be overly difficult to
  design an IO package to be completely decoupled from OpenSSL, with opt-in via
  the inclusion of a separate package).

In almost all cases, it is an antipattern to use these features outside of
high-level packages that are designed for curation. If a feature is optional, it
can almost certainly be expressed as a separate package.

[crates.io]: https://crates.io/
