## The Manifest Format

The `Cargo.toml` file for each package is called its *manifest*. Every manifest
file consists of one or more sections.

### The `[package]` section

The first section in a `Cargo.toml` is `[package]`.

```toml
[package]
name = "hello_world" # the name of the package
version = "0.1.0"    # the current version, obeying semver
authors = ["you@example.com"]
```

All three of these fields are mandatory.

#### The `version` field

Cargo bakes in the concept of [Semantic
Versioning](http://semver.org/), so make sure you follow some basic rules:

* Before you reach 1.0.0, anything goes, but if you make breaking changes,
  increment the minor version. In Rust, breaking changes include adding fields to
  structs or variants to enums.
* After 1.0.0, only make breaking changes when you increment the major version.
  Don’t break the build.
* After 1.0.0, don’t add any new public API (no new `pub` anything) in tiny
  versions. Always increment the minor version if you add any new `pub` structs,
  traits, fields, types, functions, methods or anything else.
* Use version numbers with three numeric parts such as 1.0.0 rather than 1.0.

#### The `build` field (optional)

This field specifies a file in the project root which is a [build script][1] for
building native code. More information can be found in the build script
[guide][1].

[1]: reference/build-scripts.html

```toml
[package]
# ...
build = "build.rs"
```

#### The `documentation` field (optional)

This field specifies a URL to a website hosting the crate's documentation.
If no URL is specified in the manifest file, [crates.io][cratesio] will
automatically link your crate to the corresponding [docs.rs][docsrs] page.

Documentation links from specific hosts are blacklisted. Hosts are added
to the blacklist if they are known to not be hosting documentation and are
possibly of malicious intent e.g. ad tracking networks. URLs from the
following hosts are blacklisted:

* rust-ci.org

Documentation URLs from blacklisted hosts will not appear on crates.io, and
may be replaced by docs.rs links.

[docsrs]: https://docs.rs/
[cratesio]: https://crates.io/

#### The `exclude` and `include` fields (optional)

You can explicitly specify to Cargo that a set of [globs][globs] should be
ignored or included for the purposes of packaging and rebuilding a package. The
globs specified in the `exclude` field identify a set of files that are not
included when a package is published as well as ignored for the purposes of
detecting when to rebuild a package, and the globs in `include` specify files
that are explicitly included.

If a VCS is being used for a package, the `exclude` field will be seeded with
the VCS’ ignore settings (`.gitignore` for git for example).

```toml
[package]
# ...
exclude = ["build/**/*.o", "doc/**/*.html"]
```

```toml
[package]
# ...
include = ["src/**/*", "Cargo.toml"]
```

The options are mutually exclusive: setting `include` will override an
`exclude`. Note that `include` must be an exhaustive list of files as otherwise
necessary source files may not be included.

[globs]: http://doc.rust-lang.org/glob/glob/struct.Pattern.html

#### Migrating to `gitignore`-like pattern matching

The current interpretation of these configs is based on UNIX Globs, as
implemented in the [`glob` crate](https://crates.io/crates/glob). We want
Cargo's `include` and `exclude` configs to work as similar to `gitignore` as
possible. [The `gitignore` specification](https://git-scm.com/docs/gitignore) is
also based on Globs, but has a bunch of additional features that enable easier
pattern writing and more control. Therefore, we are migrating the interpretation
for the rules of these configs to use the [`ignore`
crate](https://crates.io/crates/ignore), and treat them each rule as a single
line in a `gitignore` file. See [the tracking
issue](https://github.com/rust-lang/cargo/issues/4268) for more details on the
migration.

#### The `publish`  field (optional)

The `publish` field can be used to prevent a package from being published to a
package registry (like *crates.io*) by mistake.

```toml
[package]
# ...
publish = false
```

#### The `workspace`  field (optional)

The `workspace` field can be used to configure the workspace that this package
will be a member of. If not specified this will be inferred as the first
Cargo.toml with `[workspace]` upwards in the filesystem.

```toml
[package]
# ...
workspace = "path/to/workspace/root"
```

For more information, see the documentation for the workspace table below.

#### Package metadata

There are a number of optional metadata fields also accepted under the
`[package]` section:

```toml
[package]
# ...

# A short blurb about the package. This is not rendered in any format when
# uploaded to crates.io (aka this is not markdown).
description = "..."

# These URLs point to more information about the package. These are
# intended to be webviews of the relevant data, not necessarily compatible
# with VCS tools and the like.
documentation = "..."
homepage = "..."
repository = "..."

# This points to a file under the package root (relative to this `Cargo.toml`).
# The contents of this file are stored and indexed in the registry.
# crates.io will render this file and place the result on the crate's page.
readme = "..."

# This is a list of up to five keywords that describe this crate. Keywords
# are searchable on crates.io, and you may choose any words that would
# help someone find this crate.
keywords = ["...", "..."]

# This is a list of up to five categories where this crate would fit.
# Categories are a fixed list available at crates.io/category_slugs, and
# they must match exactly.
categories = ["...", "..."]

# This is a string description of the license for this package. Currently
# crates.io will validate the license provided against a whitelist of known
# license identifiers from http://spdx.org/licenses/. Multiple licenses can be
# separated with a `/`.
license = "..."

# If a project is using a nonstandard license, then this key may be specified in
# lieu of the above key and must point to a file relative to this manifest
# (similar to the readme key).
license-file = "..."

# Optional specification of badges to be displayed on crates.io.
#
# - The badges pertaining to build status that are currently available are
#   Appveyor, CircleCI, GitLab, and TravisCI.
# - Available badges pertaining to code test coverage are Codecov and
#   Coveralls.
# - There are also maintenance-related badges basesed on isitmaintained.com
#   which state the issue resolution time, percent of open issues, and future
#   maintenance intentions.
#
# If a `repository` key is required, this refers to a repository in
# `user/repo` format.
[badges]

# Appveyor: `repository` is required. `branch` is optional; default is `master`
# `service` is optional; valid values are `github` (default), `bitbucket`, and
# `gitlab`.
appveyor = { repository = "...", branch = "master", service = "github" }

# Circle CI: `repository` is required. `branch` is optional; default is `master`
circle-ci = { repository = "...", branch = "master" }

# GitLab: `repository` is required. `branch` is optional; default is `master`
gitlab = { repository = "...", branch = "master" }

# Travis CI: `repository` in format "<user>/<project>" is required.
# `branch` is optional; default is `master`
travis-ci = { repository = "...", branch = "master" }

# Codecov: `repository` is required. `branch` is optional; default is `master`
# `service` is optional; valid values are `github` (default), `bitbucket`, and
# `gitlab`.
codecov = { repository = "...", branch = "master", service = "github" }

# Coveralls: `repository` is required. `branch` is optional; default is `master`
# `service` is optional; valid values are `github` (default) and `bitbucket`.
coveralls = { repository = "...", branch = "master", service = "github" }

# Is it maintained resolution time: `repository` is required.
is-it-maintained-issue-resolution = { repository = "..." }

# Is it maintained percentage of open issues: `repository` is required.
is-it-maintained-open-issues = { repository = "..." }

# Maintenance: `status` is required Available options are `actively-developed`,
# `passively-maintained`, `as-is`, `none`, `experimental`, `looking-for-maintainer`
# and `deprecated`.
maintenance = { status = "..." }
```

The [crates.io](https://crates.io) registry will render the description, display
the license, link to the three URLs and categorize by the keywords. These keys
provide useful information to users of the registry and also influence the
search ranking of a crate. It is highly discouraged to omit everything in a
published crate.

#### The `metadata` table (optional)

Cargo by default will warn about unused keys in `Cargo.toml` to assist in
detecting typos and such. The `package.metadata` table, however, is completely
ignored by Cargo and will not be warned about. This section can be used for
tools which would like to store project configuration in `Cargo.toml`. For
example:

```toml
[package]
name = "..."
# ...

# Metadata used when generating an Android APK, for example.
[package.metadata.android]
package-name = "my-awesome-android-app"
assets = "path/to/static"
```

### Dependency sections

See the [specifying dependencies page](reference/specifying-dependencies.html) for
information on the `[dependencies]`, `[dev-dependencies]`,
`[build-dependencies]`, and target-specific `[target.*.dependencies]` sections.

### The `[profile.*]` sections

Cargo supports custom configuration of how rustc is invoked through profiles at
the top level. Any manifest may declare a profile, but only the top level
project’s profiles are actually read. All dependencies’ profiles will be
overridden. This is done so the top-level project has control over how its
dependencies are compiled.

There are five currently supported profile names, all of which have the same
configuration available to them. Listed below is the configuration available,
along with the defaults for each profile.

```toml
# The development profile, used for `cargo build`.
[profile.dev]
opt-level = 0      # controls the `--opt-level` the compiler builds with.
                   # 0-1 is good for debugging. 2 is well-optimized. Max is 3.
debug = true       # include debug information (debug symbols). Equivalent to
                   # `-C debuginfo=2` compiler flag.
rpath = false      # controls whether compiler should set loader paths.
                   # If true, passes `-C rpath` flag to the compiler.
lto = false        # Link Time Optimization usually reduces size of binaries
                   # and static libraries. Increases compilation time.
                   # If true, passes `-C lto` flag to the compiler.
debug-assertions = true # controls whether debug assertions are enabled
                   # (e.g. debug_assert!() and arithmetic overflow checks)
codegen-units = 1  # if > 1 enables parallel code generation which improves
                   # compile times, but prevents some optimizations.
                   # Passes `-C codegen-units`. Ignored when `lto = true`.
panic = 'unwind'   # panic strategy (`-C panic=...`), can also be 'abort'

# The release profile, used for `cargo build --release`.
[profile.release]
opt-level = 3
debug = false
rpath = false
lto = false
debug-assertions = false
codegen-units = 1
panic = 'unwind'

# The testing profile, used for `cargo test`.
[profile.test]
opt-level = 0
debug = 2
rpath = false
lto = false
debug-assertions = true
codegen-units = 1
panic = 'unwind'

# The benchmarking profile, used for `cargo bench`.
[profile.bench]
opt-level = 3
debug = false
rpath = false
lto = false
debug-assertions = false
codegen-units = 1
panic = 'unwind'

# The documentation profile, used for `cargo doc`.
[profile.doc]
opt-level = 0
debug = 2
rpath = false
lto = false
debug-assertions = true
codegen-units = 1
panic = 'unwind'
```

### The `[features]` section

Cargo supports features to allow expression of:

* conditional compilation options (usable through `cfg` attributes);
* optional dependencies, which enhance a package, but are not required; and
* clusters of optional dependencies, such as `postgres`, that would include the
  `postgres` package, the `postgres-macros` package, and possibly other packages
  (such as development-time mocking libraries, debugging tools, etc.).

A feature of a package is either an optional dependency, or a set of other
features. The format for specifying features is:

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

#### Rules

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

#### Usage in end products

One major use-case for this feature is specifying optional features in
end-products. For example, the Servo project may want to include optional
features that people can enable or disable when they build it.

In that case, Servo will describe features in its `Cargo.toml` and they can be
enabled using command-line flags:

```shell
$ cargo build --release --features "shumway pdf"
```

Default features could be excluded using `--no-default-features`.

#### Usage in packages

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

### The `[workspace]` section

Projects can define a workspace which is a set of crates that will all share the
same `Cargo.lock` and output directory. The `[workspace]` table can be defined
as:

```toml
[workspace]

# Optional key, inferred if not present
members = ["path/to/member1", "path/to/member2", "path/to/member3/*"]

# Optional key, empty if not present
exclude = ["path1", "path/to/dir2"]
```

Workspaces were added to Cargo as part of [RFC 1525] and have a number of
properties:

* A workspace can contain multiple crates where one of them is the *root crate*.
* The *root crate*'s `Cargo.toml` contains the `[workspace]` table, but is not
  required to have other configuration.
* Whenever any crate in the workspace is compiled, output is placed in the
  *workspace root*. i.e. next to the *root crate*'s `Cargo.toml`.
* The lock file for all crates in the workspace resides in the *workspace root*.
* The `[patch]` and `[replace]` sections in `Cargo.toml` are only recognized
  in the *root crate*'s manifest, and ignored in member crates' manifests.

[RFC 1525]: https://github.com/rust-lang/rfcs/blob/master/text/1525-cargo-workspace.md

The *root crate* of a workspace, indicated by the presence of `[workspace]` in
its manifest, is responsible for defining the entire workspace. All `path`
dependencies residing in the workspace directory become members. You can add
additional packages to the workspace by listing them in the `members` key. Note
that members of the workspaces listed explicitly will also have their path
dependencies included in the workspace. Sometimes a project may have a lot of
workspace members and it can be onerous to keep up to date. The path dependency
can also use [globs][globs] to match multiple paths. Finally, the `exclude`
key can be used to blacklist paths from being included in a workspace. This can
be useful if some path dependencies aren't desired to be in the workspace at
all.

The `package.workspace` manifest key (described above) is used in member crates
to point at a workspace's root crate. If this key is omitted then it is inferred
to be the first crate whose manifest contains `[workspace]` upwards in the
filesystem.

A crate may either specify `package.workspace` or specify `[workspace]`. That
is, a crate cannot both be a root crate in a workspace (contain `[workspace]`)
and also be a member crate of another workspace (contain `package.workspace`).

Most of the time workspaces will not need to be dealt with as `cargo new` and
`cargo init` will handle workspace configuration automatically.

#### Virtual Manifest

In workspace manifests, if the `package` table is present, the workspace root
crate will be treated as a normal package, as well as a worksapce. If the
`package` table is not present in a worksapce manifest, it is called a *virtual
manifest*.

When working with *virtual manifests*, package-related cargo commands, like
`cargo build`, won't be available anymore. But, most of such commands support
the `--all` option, will execute the command for all the non-virtual manifest in
the workspace.

#TODO: move this to a more appropriate place
### The project layout

If your project is an executable, name the main source file `src/main.rs`. If it
is a library, name the main source file `src/lib.rs`.

Cargo will also treat any files located in `src/bin/*.rs` as executables. If your
executable consists of more than just one source file, you might also use a directory
inside `src/bin` containing a `main.rs` file which will be treated as an executable
with a name of the parent directory.
Do note, however, once you add a `[[bin]]` section ([see
below](#configuring-a-target)), Cargo will no longer automatically build files
located in `src/bin/*.rs`.  Instead you must create a `[[bin]]` section for
each file you want to build.

Your project can optionally contain folders named `examples`, `tests`, and
`benches`, which Cargo will treat as containing examples,
integration tests, and benchmarks respectively.

```shell
▾ src/           # directory containing source files
  lib.rs         # the main entry point for libraries and packages
  main.rs        # the main entry point for projects producing executables
  ▾ bin/         # (optional) directory containing additional executables
    *.rs
  ▾ */           # (optional) directories containing multi-file executables
    main.rs
▾ examples/      # (optional) examples
  *.rs
▾ tests/         # (optional) integration tests
  *.rs
▾ benches/       # (optional) benchmarks
  *.rs
```

To structure your code after you've created the files and folders for your
project, you should remember to use Rust's module system, which you can read
about in [the book](https://doc.rust-lang.org/book/crates-and-modules.html).

### Examples

Files located under `examples` are example uses of the functionality provided by
the library. When compiled, they are placed in the `target/examples` directory.

They can compile either as executables (with a `main()` function) or libraries
and pull in the library by using `extern crate <library-name>`. They are
compiled when you run your tests to protect them from bitrotting.

You can run individual executable examples with the command `cargo run --example
<example-name>`.

Specify `crate-type` to make an example be compiled as a library:

```toml
[[example]]
name = "foo"
crate-type = ["staticlib"]
```

You can build individual library examples with the command `cargo build
--example <example-name>`.

### Tests

When you run `cargo test`, Cargo will:

* compile and run your library’s unit tests, which are in the files reachable
  from `lib.rs` (naturally, any sections marked with `#[cfg(test)]` will be
  considered at this stage);
* compile and run your library’s documentation tests, which are embedded inside
  of documentation blocks;
* compile and run your library’s [integration tests](#integration-tests); and
* compile your library’s examples.

#### Integration tests

Each file in `tests/*.rs` is an integration test. When you run `cargo test`,
Cargo will compile each of these files as a separate crate. The crate can link
to your library by using `extern crate <library-name>`, like any other code that
depends on it.

Cargo will not automatically compile files inside subdirectories of `tests`, but
an integration test can import modules from these directories as usual. For
example, if you want several integration tests to share some code, you can put
the shared code in `tests/common/mod.rs` and then put `mod common;` in each of
the test files.

### Configuring a target

All of the  `[[bin]]`, `[lib]`, `[[bench]]`, `[[test]]`, and `[[example]]`
sections support similar configuration for specifying how a target should be
built. The double-bracket sections like `[[bin]]` are array-of-table of
[TOML](https://github.com/toml-lang/toml#array-of-tables), which means you can
write more than one `[[bin]]` section to make several executables in your crate.

The example below uses `[lib]`, but it also applies to all other sections
as well. All values listed are the defaults for that option unless otherwise
specified.

```toml
[package]
# ...

[lib]
# The name of a target is the name of the library that will be generated. This
# is defaulted to the name of the package or project, with any dashes replaced
# with underscores. (Rust `extern crate` declarations reference this name;
# therefore the value must be a valid Rust identifier to be usable.)
name = "foo"

# This field points at where the crate is located, relative to the `Cargo.toml`.
path = "src/lib.rs"

# A flag for enabling unit tests for this target. This is used by `cargo test`.
test = true

# A flag for enabling documentation tests for this target. This is only relevant
# for libraries, it has no effect on other sections. This is used by
# `cargo test`.
doctest = true

# A flag for enabling benchmarks for this target. This is used by `cargo bench`.
bench = true

# A flag for enabling documentation of this target. This is used by `cargo doc`.
doc = true

# If the target is meant to be a compiler plugin, this field must be set to true
# for Cargo to correctly compile it and make it available for all dependencies.
plugin = false

# If the target is meant to be a "macros 1.1" procedural macro, this field must
# be set to true.
proc-macro = false

# If set to false, `cargo test` will omit the `--test` flag to rustc, which
# stops it from generating a test harness. This is useful when the binary being
# built manages the test runner itself.
harness = true
```

#### The `required-features` field (optional)

The `required-features` field specifies which features the target needs in order
to be built. If any of the required features are not selected, the target will
be skipped. This is only relevant for the `[[bin]]`, `[[bench]]`, `[[test]]`,
and `[[example]]` sections, it has no effect on `[lib]`.

```toml
[features]
# ...
postgres = []
sqlite = []
tools = []

[[bin]]
# ...
required-features = ["postgres", "tools"]
```

#### Building dynamic or static libraries

If your project produces a library, you can specify which kind of library to
build by explicitly listing the library in your `Cargo.toml`:

```toml
# ...

[lib]
name = "..."
crate-type = ["dylib"] # could be `staticlib` as well
```

The available options are `dylib`, `rlib`, `staticlib`, `cdylib`, and
`proc-macro`. You should only use this option in a project. Cargo will always
compile packages (dependencies) based on the requirements of the project that
includes them.

You can read more about the different crate types in the
[Rust Reference Manual](https://doc.rust-lang.org/reference/linkage.html)

### The `[patch]` Section

This section of Cargo.toml can be used to [override dependencies][replace] with
other copies. The syntax is similar to the `[dependencies]` section:

```toml
[patch.crates-io]
foo = { git = 'https://github.com/example/foo' }
bar = { path = 'my/local/bar' }
```

The `[patch]` table is made of dependency-like sub-tables. Each key after
`[patch]` is a URL of the source that's being patched, or `crates-io` if
you're modifying the https://crates.io registry. In the example above
`crates-io` could be replaced with a git URL such as
`https://github.com/rust-lang-nursery/log`.

Each entry in these tables is a normal dependency specification, the same as
found in the `[dependencies]` section of the manifest. The dependencies listed
in the `[patch]` section are resolved and used to patch the source at the
URL specified. The above manifest snippet patches the `crates-io` source (e.g.
crates.io itself) with the `foo` crate and `bar` crate.

Sources can be patched with versions of crates that do not exist, and they can
also be patched with versions of crates that already exist. If a source is
patched with a crate version that already exists in the source, then the
source's original crate is replaced.

More information about overriding dependencies can be found in the [overriding
dependencies][replace] section of the documentation and [RFC 1969] for the
technical specification of this feature. (Note that the `[patch]` feature will
first become available in Rust 1.21, set to be released on 2017-10-12.)

[RFC 1969]: https://github.com/rust-lang/rfcs/pull/1969
[replace]: reference/specifying-dependencies.html#overriding-dependencies

### The `[replace]` Section

This section of Cargo.toml can be used to [override dependencies][replace] with
other copies. The syntax is similar to the `[dependencies]` section:

```toml
[replace]
"foo:0.1.0" = { git = 'https://github.com/example/foo' }
"bar:1.0.2" = { path = 'my/local/bar' }
```

Each key in the `[replace]` table is a [package id
specification](reference/pkgid-spec.html) which allows arbitrarily choosing a node in the
dependency graph to override. The value of each key is the same as the
`[dependencies]` syntax for specifying dependencies, except that you can't
specify features. Note that when a crate is overridden the copy it's overridden
with must have both the same name and version, but it can come from a different
source (e.g. git or a local path).

More information about overriding dependencies can be found in the [overriding
dependencies][replace] section of the documentation.
