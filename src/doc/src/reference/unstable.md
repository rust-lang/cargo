## Unstable Features

Experimental Cargo features are only available on the nightly channel. You
typically use one of the `-Z` flags to enable them. Run `cargo -Z help` to
see a list of flags available.

`-Z unstable-options` is a generic flag for enabling other unstable
command-line flags. Options requiring this will be called out below.

Some unstable features will require you to specify the `cargo-features` key in
`Cargo.toml`.

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
* Tracking Issue: [#6790](https://github.com/rust-lang/cargo/issues/6790)

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

# Build scripts or proc-macros and their dependencies will be compiled with
# `-Copt-level=3`. By default, they use the same rules as the rest of the
# profile.
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
* Tracking Issue: [#5565](https://github.com/rust-lang/cargo/issues/5565)

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
* Tracking Issue: [#5579](https://github.com/rust-lang/cargo/issues/5579)

The `--build-plan` argument for the `build` command will output JSON with
information about which commands would be run without actually executing
anything. This can be useful when integrating with another build tool.
Example:

```
cargo +nightly build --build-plan -Z unstable-options
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

### install-upgrade
* Tracking Issue: [#6797](https://github.com/rust-lang/cargo/issues/6797)

The `install-upgrade` feature changes the behavior of `cargo install` so that
it will reinstall a package if it is not "up-to-date". If it is "up-to-date",
it will do nothing and exit with success instead of failing. Example:

```
cargo +nightly install foo -Z install-upgrade
```

Cargo tracks some information to determine if a package is "up-to-date",
including:

- The package version and source.
- The set of binary names installed.
- The chosen features.
- The release mode (`--debug`).
- The target (`--target`).

If any of these values change, then Cargo will reinstall the package.

Installation will still fail if a different package installs a binary of the
same name. `--force` may be used to unconditionally reinstall the package.

Installing with `--path` will always build and install, unless there are
conflicting binaries from another package.

Additionally, a new flag `--no-track` is available to prevent `cargo install`
from writing tracking information in `$CARGO_HOME` about which packages are
installed.

### public-dependency
* Tracking Issue: [#44663](https://github.com/rust-lang/rust/issues/44663)

The 'public-dependency' feature allows marking dependencies as 'public'
or 'private'. When this feature is enabled, additional information is passed to rustc to allow
the 'exported_private_dependencies' lint to function properly.

This requires the appropriate key to be set in `cargo-features`:

```toml
cargo-features = ["public-dependency"]

[dependencies]
my_dep = { version = "1.2.3", public = true }
private_dep = "2.0.0" # Will be 'private' by default
```

### cache-messages
* Tracking Issue: [#6986](https://github.com/rust-lang/cargo/issues/6986)

The `cache-messages` feature causes Cargo to cache the messages generated by
the compiler. This is primarily useful if a crate compiles successfully with
warnings. Previously, re-running Cargo would not display any output. With the
`cache-messages` feature, it will quickly redisplay the previous warnings.

```
cargo +nightly check -Z cache-messages
```

This works with any command that runs the compiler (`build`, `check`, `test`,
etc.).

This also changes the way Cargo interacts with the compiler, helping to
prevent interleaved messages when multiple crates attempt to display a message
at the same time.
