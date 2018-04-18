## Unstable Features

Experimental Cargo features are only available on the nightly channel.  You
typically use one of the `-Z` flags to enable them.  Run `cargo -Z help` to
see a list of flags available.

`-Z unstable-options` is a generic flag for enabling other unstable
command-line flags.  Options requiring this will be called out below.

Some unstable features will require you to specify the `cargo-features` key in
`Cargo.toml`.

### Alternate Registries
* RFC: [#2141](https://github.com/rust-lang/rfcs/blob/master/text/2141-alternative-registries.md)
* Tracking Issue: [rust-lang/rust#44931](https://github.com/rust-lang/rust/issues/44931)

Alternate registries allow you to use registries other than crates.io.

The name of a registry is defined in `.cargo/config` under the `registries`
table:

```toml
[registries]
my-registry = { index = "https://my-intranet:8080/index" }
```

Authentication information for alternate registries can be added to
`.cargo/credentials`:

```toml
[my-registry]
token = "api-token"
```

Inside `Cargo.toml` you can specify which registry a dependency comes from
using the `registry` key. First you need to include the appropriate
`cargo-features` at the top of the file:

```toml
cargo-features = ["alternative-registries"]

[package]
...

[dependencies]
other-create = { version = "1.0", registry = "my-registry"}
```

A `--registry` flag has been added to commands that interact with registries
such as `publish`, `login`, etc.  Example:

```
cargo +nightly publish -Z unstable-options --registry my-registry
```

The `publish` field in `Cargo.toml` has been extended to accept a list of
registries that will restrict publishing only to those registries.

```toml
[package]
...
publish = ["my-registry"]
```


### rename-dependency
* Original Issue: [#1311](https://github.com/rust-lang/cargo/issues/1311)
* PR: [#4953](https://github.com/rust-lang/cargo/pull/4953)

The rename-dependency feature allows you to import a dependency
with a different name from the source.  This can be useful in a few scenarios:

* Depending on crates with the same name from different registries.
* Depending on multiple versions of a crate.
* Avoid needing `extern crate foo as bar` in Rust source.

Just include the `package` key to specify the actual name of the dependency.
You must include `cargo-features` at the top of your `Cargo.toml`.

```toml
cargo-features = ["rename-dependency"]

[package]
name = "mypackage"
version = "0.0.1"

[dependencies]
foo = "0.1"
bar = { version = "0.1", registry = "custom", package = "foo" }
baz = { git = "https://github.com/example/project", package = "foo" }
```

In this example, three crates are now available in your Rust code:

```rust
extern crate foo;  // crates.io
extern crate bar;  // registry `custom`
extern crate baz;  // git repository
```


### publish-lockfile
* Original Issue: [#2263](https://github.com/rust-lang/cargo/issues/2263)
* PR: [#5093](https://github.com/rust-lang/cargo/pull/5093)

When creating a `.crate` file for distribution, Cargo has historically
not included the `Cargo.lock` file.  This can cause problems with
using `cargo install` with a binary.  You can specify that your package
should include the `Cargo.lock` file when using `cargo package` or `cargo publish`
by specifying the `publish-lockfile` key in `Cargo.toml`.  This also requires the
appropriate `cargo-features`:

```toml
cargo-features = ["publish-lockfile"]

[project]
...
publish-lockfile = true
```


### Offline Mode
* Original Issue: [#4686](https://github.com/rust-lang/cargo/issues/4686)

The `-Z offline` flag prevents Cargo from attempting to access the network for
any reason.  Typically Cargo will stop with an error if it wants to access the
network and it is not available.

Beware that this may result in different dependency resolution than online
mode.  Cargo will restrict itself to crates that are available locally, even
if there might be a newer version as indicated in the local copy of the index.

### no-index-update
* Original Issue: [#3479](https://github.com/rust-lang/cargo/issues/3479)

The `-Z no-index-update` flag ensures that Cargo does not attempt to update
the registry index.  This is intended for tools such as Crater that issue many
Cargo commands, and you want to avoid the network latency for updating the
index each time.

### avoid-dev-deps
* Original Issue: [#4988](https://github.com/rust-lang/cargo/issues/4988)
* Stabilization Issue: [#5133](https://github.com/rust-lang/cargo/issues/5133)

When running commands such as `cargo install` or `cargo build`, Cargo
currently requires dev-dependencies to be downloaded, even if they are not
used.  The `-Z avoid-dev-deps` flag allows Cargo to avoid downloading
dev-dependencies if they are not needed.  The `Cargo.lock` file will not be
generated if dev-dependencies are skipped.

### minimal-versions
* Original Issue: [#4100](https://github.com/rust-lang/cargo/issues/4100)

When a `Cargo.lock` file is generated, the `-Z minimal-versions` flag will
resolve the dependencies to the minimum semver version that will satisfy the
requirements (instead of the greatest version).

The intended use-case of this flag is to check, during continuous integration,
that the versions specified in Cargo.toml are a correct reflection of the
minimum versions that you are actually using. That is, if Cargo.toml says
`foo = "1.0.0"` that you don't accidentally depend on features added only in
`foo 1.5.0`.

### out-dir
* Original Issue: [#4875](https://github.com/rust-lang/cargo/issues/4875)

This feature allows you to specify the directory where artifacts will be
copied to after they are built.  Typically artifacts are only written to the
`target/release` or `target/debug` directories.  However, determining the
exact filename can be tricky since you need to parse JSON output. The
`--out-dir` flag makes it easier to predictably access the artifacts. Note
that the artifacts are copied, so the originals are still in the `target`
directory.  Example:

```
cargo +nightly build --out-dir=out -Z unstable-options
```


### Edition
* Tracking Issue: [rust-lang/rust#44581](https://github.com/rust-lang/rust/issues/44581)
* RFC: [#2052](https://github.com/rust-lang/rfcs/blob/master/text/2052-epochs.md)

You can opt in to a specific Rust Edition for your package with the `rust` key
in `Cargo.toml`.  If you don't specify the edition, it will default to 2015.
You need to include the appropriate `cargo-features`:

```toml
cargo-features = ["edition"]

[package]
...
rust = "2018"
```


### Profile Overrides
* Tracking Issue: [rust-lang/rust#48683](https://github.com/rust-lang/rust/issues/48683)
* RFC: [#2282](https://github.com/rust-lang/rfcs/blob/master/text/2282-profile-dependencies.md)

Profiles can be overridden for specific packages and custom build scripts.
The general format looks like this:

```toml
cargo-features = ["profile-overrides"]

[package]
...

[profile.dev]
opt-level = 0
debug = true

# the `image` crate will be compiled with -Copt-level=3
[profile.dev.overrides.image]
opt-level = 3

# All dependencies (but not this crate itself) will be compiled
# with -Copt-level=2 . This includes build dependencies.
[profile.dev.overrides."*"]
opt-level = 2

# Build scripts and their dependencies will be compiled with -Copt-level=3
# By default, build scripts use the same rules as the rest of the profile
[profile.dev.build_override]
opt-level = 3
```

Overrides can only be specified for dev and release profiles.
