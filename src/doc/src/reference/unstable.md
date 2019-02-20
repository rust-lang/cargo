## Unstable Features

Experimental Cargo features are only available on the nightly channel. You
typically use one of the `-Z` flags to enable them. Run `cargo -Z help` to
see a list of flags available.

`-Z unstable-options` is a generic flag for enabling other unstable
command-line flags. Options requiring this will be called out below.

Some unstable features will require you to specify the `cargo-features` key in
`Cargo.toml`.

### publish-lockfile
* Original Issue: [#2263](https://github.com/rust-lang/cargo/issues/2263)
* PR: [#5093](https://github.com/rust-lang/cargo/pull/5093)
* Tracking Issue: [#5654](https://github.com/rust-lang/cargo/issues/5654)

When creating a `.crate` file for distribution, Cargo has historically
not included the `Cargo.lock` file. This can cause problems with
using `cargo install` with a binary. You can specify that your package
should include the `Cargo.lock` file when using `cargo package` or `cargo publish`
by specifying the `publish-lockfile` key in `Cargo.toml`. This also requires the
appropriate `cargo-features`:

```toml
cargo-features = ["publish-lockfile"]

[package]
...
publish-lockfile = true
```


### Offline Mode
* Original Issue: [#4686](https://github.com/rust-lang/cargo/issues/4686)
* Tracking Issue: [#5655](https://github.com/rust-lang/cargo/issues/5655)

The `-Z offline` flag prevents Cargo from attempting to access the network for
any reason. Typically Cargo will stop with an error if it wants to access the
network and it is not available.

Beware that this may result in different dependency resolution than online
mode. Cargo will restrict itself to crates that are available locally, even
if there might be a newer version as indicated in the local copy of the index.

### no-index-update
* Original Issue: [#3479](https://github.com/rust-lang/cargo/issues/3479)

The `-Z no-index-update` flag ensures that Cargo does not attempt to update
the registry index. This is intended for tools such as Crater that issue many
Cargo commands, and you want to avoid the network latency for updating the
index each time.

### avoid-dev-deps
* Original Issue: [#4988](https://github.com/rust-lang/cargo/issues/4988)
* Stabilization Issue: [#5133](https://github.com/rust-lang/cargo/issues/5133)

When running commands such as `cargo install` or `cargo build`, Cargo
currently requires dev-dependencies to be downloaded, even if they are not
used. The `-Z avoid-dev-deps` flag allows Cargo to avoid downloading
dev-dependencies if they are not needed. The `Cargo.lock` file will not be
generated if dev-dependencies are skipped.

### minimal-versions
* Original Issue: [#4100](https://github.com/rust-lang/cargo/issues/4100)
* Tracking Issue: [#5657](https://github.com/rust-lang/cargo/issues/5657)

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
copied to after they are built. Typically artifacts are only written to the
`target/release` or `target/debug` directories. However, determining the
exact filename can be tricky since you need to parse JSON output. The
`--out-dir` flag makes it easier to predictably access the artifacts. Note
that the artifacts are copied, so the originals are still in the `target`
directory. Example:

```
cargo +nightly build --out-dir=out -Z unstable-options
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

# All dependencies (but not this crate itself or any workspace member)
# will be compiled with -Copt-level=2 . This includes build dependencies.
[profile.dev.overrides."*"]
opt-level = 2

# Build scripts and their dependencies will be compiled with -Copt-level=3
# By default, build scripts use the same rules as the rest of the profile
[profile.dev.build-override]
opt-level = 3
```

Overrides can only be specified for dev and release profiles.


### Config Profiles
* Tracking Issue: [rust-lang/rust#48683](https://github.com/rust-lang/rust/issues/48683)
* RFC: [#2282](https://github.com/rust-lang/rfcs/blob/master/text/2282-profile-dependencies.md)

Profiles can be specified in `.cargo/config` files. The `-Z config-profile`
command-line flag is required to use this feature. The format is the same as
in a `Cargo.toml` manifest. If found in multiple config files, settings will
be merged using the regular [config hierarchy](reference/config.html#hierarchical-structure).
Config settings take precedence over manifest settings.

```toml
[profile.dev]
opt-level = 3
```

```
cargo +nightly build -Z config-profile
```


### Namespaced features
* Original issue: [#1286](https://github.com/rust-lang/cargo/issues/1286)
* Tracking Issue: [rust-lang/cargo#5565](https://github.com/rust-lang/cargo/issues/5565)

Currently, it is not possible to have a feature and a dependency with the same
name in the manifest. If you set `namespaced-features` to `true`, the namespaces
for features and dependencies are separated. The effect of this is that, in the
feature requirements, dependencies have to be prefixed with `crate:`. Like this:

```toml
[package]
namespaced-features = true

[features]
bar = ["crate:baz", "foo"]
foo = []

[dependencies]
baz = { version = "0.1", optional = true }
```

To prevent unnecessary boilerplate from having to explicitly declare features
for each optional dependency, implicit features get created for any optional
dependencies where a feature of the same name is not defined. However, if
a feature of the same name as a dependency is defined, that feature must
include the dependency as a requirement, as `foo = ["crate:foo"]`.


### Build-plan
* Tracking Issue: [rust-lang/cargo#5579](https://github.com/rust-lang/cargo/issues/5579)

The `--build-plan` argument for the `build` command will output JSON with
information about which commands would be run without actually executing
anything. This can be useful when integrating with another build tool.
Example:

```
cargo +nightly build --build-plan -Z unstable-options
```

### default-run
* Original issue: [#2200](https://github.com/rust-lang/cargo/issues/2200)

The `default-run` option in the `[package]` section of the manifest can be used
to specify a default binary picked by `cargo run`. For example, when there is
both `src/bin/a.rs` and `src/bin/b.rs`:

```toml
[package]
default-run = "a"
```

### Metabuild
* Tracking Issue: [rust-lang/rust#49803](https://github.com/rust-lang/rust/issues/49803)
* RFC: [#2196](https://github.com/rust-lang/rfcs/blob/master/text/2196-metabuild.md)

Metabuild is a feature to have declarative build scripts. Instead of writing
a `build.rs` script, you specify a list of build dependencies in the
`metabuild` key in `Cargo.toml`. A build script is automatically generated
that runs each build dependency in order. Metabuild packages can then read
metadata from `Cargo.toml` to specify their behavior.

Include `cargo-features` at the top of `Cargo.toml`, a `metabuild` key in the
`package`, list the dependencies in `build-dependencies`, and add any metadata
that the metabuild packages require under `package.metadata`. Example:

```toml
cargo-features = ["metabuild"]

[package]
name = "mypackage"
version = "0.0.1"
metabuild = ["foo", "bar"]

[build-dependencies]
foo = "1.0"
bar = "1.0"

[package.metadata.foo]
extra-info = "qwerty"
```

Metabuild packages should have a public function called `metabuild` that
performs the same actions as a regular `build.rs` script would perform.
