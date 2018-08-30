## Configuration

This document will explain how Cargo’s configuration system works, as well as
available keys or configuration.  For configuration of a project through its
manifest, see the [manifest format](reference/manifest.html).

### Hierarchical structure


Cargo allows local configuration for a particular project as well as global
configuration, like git. Cargo extends this to a hierarchical strategy.
If, for example, Cargo were invoked in `/projects/foo/bar/baz`, then the
following configuration files would be probed for and unified in this order:

* `/projects/foo/bar/baz/.cargo/config`
* `/projects/foo/bar/.cargo/config`
* `/projects/foo/.cargo/config`
* `/projects/.cargo/config`
* `/.cargo/config`
* `$HOME/.cargo/config`

With this structure, you can specify configuration per-project, and even
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

# By default `cargo new` will initialize a new Git repository. This key can be
# set to `hg` to create a Mercurial repository, or `none` to disable this
# behavior.
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

# Configuration keys related to the registry
[registry]
index = "..."   # URL of the registry index (defaults to the central repository)
token = "..."   # Access token (found on the central repo’s website)

[http]
proxy = "host:port" # HTTP proxy to use for HTTP requests (defaults to none)
                    # in libcurl format, e.g. "socks5h://host:port"
timeout = 60000     # Timeout for each HTTP request, in milliseconds
cainfo = "cert.pem" # Path to Certificate Authority (CA) bundle (optional)
check-revoke = true # Indicates whether SSL certs are checked for revocation

[build]
jobs = 1                  # number of parallel jobs, defaults to # of CPUs
rustc = "rustc"           # the rust compiler tool
rustdoc = "rustdoc"       # the doc generator tool
target = "triple"         # build for the target triple
target-dir = "target"     # path of where to place all generated artifacts
rustflags = ["..", ".."]  # custom flags to pass to all compiler invocations
incremental = true        # whether or not to enable incremental compilation
dep-info-basedir = ".."   # full path for the base directory for targets in depfiles

[term]
verbose = false        # whether cargo provides verbose output
color = 'auto'         # whether cargo colorizes output

# Network configuration
[net]
retry = 2 # number of times a network call will automatically retried
git-fetch-with-cli = false  # if `true` we'll use `git`-the-CLI to fetch git repos

# Alias cargo commands. The first 3 aliases are built in. If your
# command requires grouped whitespace use the list format.
[alias]
b = "build"
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

[env]: reference/environment-variables.html
[source]: reference/source-replacement.html
