% Configuration

This document will explain how Cargo’s configuration system works, as well as
available keys or configuration.  For configuration of a project through its
manifest, see the [manifest format](manifest.html).

# Hierarchical structure

Cargo allows to have local configuration for a particular project or global
configuration (like git). Cargo also extends this ability to a hierarchical
strategy. If, for example, Cargo were invoked in `/home/foo/bar/baz`, then the
following configuration files would be probed for:

* `/home/foo/bar/baz/.cargo/config`
* `/home/foo/bar/.cargo/config`
* `/home/foo/.cargo/config`
* `/home/.cargo/config`
* `/.cargo/config`

With this structure you can specify local configuration per-project, and even
possibly check it into version control. You can also specify personal default
with a configuration file in your home directory.

# Configuration Format

All configuration is currently in the [TOML format][toml] (like the manifest),
with simple key-value pairs inside of sections (tables) which all get merged
together.

[toml]: https://github.com/toml-lang/toml

# Configuration keys

All of the following keys are optional, and their defaults are listed as their
value unless otherwise noted.

Key values that specify a tool may be given as an absolute path, a relative path
or as a pathless tool name. Absolute paths and pathless tool names are used as
given. Relative paths are resolved relative to the parent directory of the
`.cargo` directory of the config file that the value resides within.

```toml
# An array of paths to local repositories which are to be used as overrides for
# dependencies. For more information see the Cargo Guide.
paths = ["/path/to/override"]

[cargo-new]
# This is your name/email to place in the `authors` section of a new Cargo.toml
# that is generated. If not present, then `git` will be probed, and if that is
# not present then `$USER` and `$EMAIL` will be used.
name = "..."
email = "..."

# By default `cargo new` will initialize a new git repository. This key can be
# set to `none` to disable this behavior.
vcs = "none"

# For the following sections, $triple refers to any valid target triple, not the
# literal string "$triple", and it will apply whenever that target triple is
# being compiled to.
[target]
# For Cargo builds which do not mention --target, this is the linker
# which is passed to rustc (via `-C linker=`). By default this flag is not
# passed to the compiler.
linker = ".."

[target.$triple]
# Similar to the above linker configuration, but this only applies to
# when the `$triple` is being compiled for.
linker = ".."

# Configuration keys related to the registry
[registry]
index = "..."   # URL of the registry index (defaults to the central repository)
token = "..."   # Access token (found on the central repo’s website)

[http]
proxy = "..."     # HTTP proxy to use for HTTP requests (defaults to none)
timeout = 60000   # Timeout for each HTTP request, in milliseconds

[build]
jobs = 1                  # number of jobs to run by default (default to # cpus)
rustc = "rustc"           # the rust compiler tool
rustdoc = "rustdoc"       # the doc generator tool
target = "triple"         # build for the target triple
target-dir = "target"     # path of where to place all generated artifacts
rustflags = ["..", ".."]  # custom flags to pass to all compiler invocations

[term]
verbose = false        # whether cargo provides verbose output
color = 'auto'         # whether cargo colorizes output

# Network configuration
[net]
retry = 2 # number of times a network call will automatically retried
```

# Environment Variables

Cargo can also be configured through environment variables in addition to the
TOML syntax above. For each configuration key above of the form `foo.bar` the
environment variable `CARGO_FOO_BAR` can also be used to define the value. For
example the `build.jobs` key can also be defined by `CARGO_BUILD_JOBS`.

Environment variables will take precedent over TOML configuration, and currently
only integer, boolean, and string keys are supported to be defined by
environment variables.

In addition to the system above, Cargo recognizes a few other specific
[environment variables][env].

[env]: environment-variables.html
