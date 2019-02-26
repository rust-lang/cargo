## Configuration

This document will explain how Cargo’s configuration system works, as well as
available keys or configuration. For configuration of a package through its
manifest, see the [manifest format](reference/manifest.html).

### Hierarchical structure

Cargo allows local configuration for a particular package as well as global
configuration, like git. Cargo extends this to a hierarchical strategy.
If, for example, Cargo were invoked in `/projects/foo/bar/baz`, then the
following configuration files would be probed for and unified in this order:

* `/projects/foo/bar/baz/.cargo/config`
* `/projects/foo/bar/.cargo/config`
* `/projects/foo/.cargo/config`
* `/projects/.cargo/config`
* `/.cargo/config`
* `$CARGO_HOME/config` (`$CARGO_HOME` defaults to `$HOME/.cargo`)

With this structure, you can specify configuration per-package, and even
possibly check it into version control. You can also specify personal defaults
with a configuration file in your home directory.

### Configuration format

All configuration is currently in the [TOML format][toml] (like the manifest),
with simple key-value pairs inside of sections (tables) which all get merged
together.

[toml]: https://github.com/toml-lang/toml

### Configuration keys

All of the following keys are optional, and their defaults are listed as their
value unless otherwise noted.

Key values that specify a tool may be given as an absolute path, a relative path
or as a pathless tool name. Absolute paths and pathless tool names are used as
given. Relative paths are resolved relative to the parent directory of the
`.cargo` directory of the config file that the value resides within.

```toml
# An array of paths to local repositories which are to be used as overrides for
# dependencies. For more information see the Specifying Dependencies guide.
paths = ["/path/to/override"]

[cargo-new]
# This is your name/email to place in the `authors` section of a new Cargo.toml
# that is generated. If not present, then `git` will be probed, and if that is
# not present then `$USER` and `$EMAIL` will be used.
name = "..."
email = "..."

# By default `cargo new` will initialize a new Git repository. This key can
# be set to change the version control system used. Valid values are `git`,
# `hg` (for Mecurial), `pijul`, `fossil`, or `none` to disable this behavior.
vcs = "none"

# For the following sections, $triple refers to any valid target triple, not the
# literal string "$triple", and it will apply whenever that target triple is
# being compiled to. 'cfg(...)' refers to the Rust-like `#[cfg]` syntax for
# conditional compilation.
[target.$triple]
# This is the linker which is passed to rustc (via `-C linker=`) when the `$triple`
# is being compiled for. By default this flag is not passed to the compiler.
linker = ".."
# Same but for the library archiver which is passed to rustc via `-C ar=`.
ar = ".."
# If a runner is provided, compiled targets for the `$triple` will be executed
# by invoking the specified runner executable with actual target as first argument.
# This applies to `cargo run`, `cargo test` and `cargo bench` commands.
# By default compiled targets are executed directly.
runner = ".."
# custom flags to pass to all compiler invocations that target $triple
# this value overrides build.rustflags when both are present
rustflags = ["..", ".."]

[target.'cfg(...)']
# Similar for the $triple configuration, but using the `cfg` syntax.
# If several `cfg` and $triple targets are candidates, then the rustflags
# are concatenated. The `cfg` syntax only applies to rustflags, and not to
# linker.
rustflags = ["..", ".."]
# Similar for the $triple configuration, but using the `cfg` syntax.
# If one or more `cfg`s, and a $triple target are candidates, then the $triple
# will be used
# If several `cfg` are candidates, then the build will error
runner = ".."

# Configuration keys related to the registry
[registry]
index = "..."   # URL of the registry index (defaults to the index of crates.io)
default = "..." # Name of the default registry to use (can be overridden with
                # --registry)

# Configuration keys for registries other than crates.io.
# `$name` should be the name of the registry, which will be used for
# dependencies in `Cargo.toml` files and the `--registry` command-line flag.
# Registry names should only contain alphanumeric characters, `-`, or `_`.
[registries.$name]
index = "..."   # URL of the registry index

[http]
proxy = "host:port" # HTTP proxy to use for HTTP requests (defaults to none)
                    # in libcurl format, e.g., "socks5h://host:port"
timeout = 30        # Timeout for each HTTP request, in seconds
cainfo = "cert.pem" # Path to Certificate Authority (CA) bundle (optional)
check-revoke = true # Indicates whether SSL certs are checked for revocation
low-speed-limit = 5 # Lower threshold for bytes/sec (10 = default, 0 = disabled)
multiplexing = true # whether or not to use HTTP/2 multiplexing where possible

# This setting can be used to help debug what's going on with HTTP requests made
# by Cargo. When set to `true` then Cargo's normal debug logging will be filled
# in with HTTP information, which you can extract with
# `RUST_LOG=cargo::ops::registry=debug` (and `trace` may print more).
#
# Be wary when posting these logs elsewhere though, it may be the case that a
# header has an authentication token in it you don't want leaked! Be sure to
# briefly review logs before posting them.
debug = false

[build]
jobs = 1                  # number of parallel jobs, defaults to # of CPUs
rustc = "rustc"           # the rust compiler tool
rustdoc = "rustdoc"       # the doc generator tool
target = "triple"         # build for the target triple (ignored by `cargo install`)
target-dir = "target"     # path of where to place all generated artifacts
rustflags = ["..", ".."]  # custom flags to pass to all compiler invocations
rustdocflags = ["..", ".."]  # custom flags to pass to rustdoc
incremental = true        # whether or not to enable incremental compilation
                          # If `incremental` is not set, then the value from
                          # the profile is used.
dep-info-basedir = ".."   # full path for the base directory for targets in depfiles

[term]
verbose = false        # whether cargo provides verbose output
color = 'auto'         # whether cargo colorizes output

# Network configuration
[net]
retry = 2 # number of times a network call will automatically retried
git-fetch-with-cli = false  # if `true` we'll use `git`-the-CLI to fetch git repos

# Alias cargo commands. The first 4 aliases are built in. If your
# command requires grouped whitespace use the list format.
[alias]
b = "build"
c = "check"
t = "test"
r = "run"
rr = "run --release"
space_example = ["run", "--release", "--", "\"command list\""]
```

### Environment variables

Cargo can also be configured through environment variables in addition to the
TOML syntax above. For each configuration key above of the form `foo.bar` the
environment variable `CARGO_FOO_BAR` can also be used to define the value. For
example the `build.jobs` key can also be defined by `CARGO_BUILD_JOBS`.

Environment variables will take precedent over TOML configuration, and currently
only integer, boolean, and string keys are supported to be defined by
environment variables. This means that [source replacement][source], which is expressed by
tables, cannot be configured through environment variables.

In addition to the system above, Cargo recognizes a few other specific
[environment variables][env].

### Credentials

Configuration values with sensitive information are stored in the
`$CARGO_HOME/credentials` file. This file is automatically created and updated
by [`cargo login`]. It follows the same format as Cargo config files.

```toml
[registry]
token = "..."   # Access token for crates.io

# `$name` should be a registry name (see above for more information about
# configuring registries).
[registries.$name]
token = "..."   # Access token for the named registry
```

Tokens are used by some Cargo commands such as [`cargo publish`] for
authenticating with remote registries. Care should be taken to protect the
tokens and to keep them secret.

As with most other config values, tokens may be specified with environment
variables. The token for crates.io may be specified with the
`CARGO_REGISTRY_TOKEN` environment variable. Tokens for other registries may
be specified with environment variables of the form
`CARGO_REGISTRIES_NAME_TOKEN` where `NAME` is the name of the registry in all
capital letters.

[`cargo login`]: commands/cargo-login.html
[`cargo publish`]: commands/cargo-publish.html
[env]: reference/environment-variables.html
[source]: reference/source-replacement.html
