% Configuration

This document will explain how cargo's configuration system works, as well as
available keys or configuration.  For configuration of a project through its
manifest, see the [manifest format](manifest.html).

# Hierarchical structure

Cargo allows to have local configuration for a particular project or global
configuration (like git). Cargo also extends this ability to a hierarchical
strategy. If, for example, cargo were invoked in `/home/foo/bar/baz`, then the
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

All configuration is currently in the TOML format (like the manifest), with
simple key-value pairs inside of sections (tables) which all get merged
together.

# Configuration keys

All of the following keys are optional, and their defaults are listed as their
value unless otherwise noted.

```toml
# An array of paths to local repositories which are to be used as overrides for
# dependencies. For more information see the Cargo Guide.
paths = [ "/path/to/override" ]

[cargo-new]
# This is your name/email to place in the `authors` section of a new Cargo.toml
# that is generated. If not present, then `git` will be probed, and if that is
# not present then `$USER` will be used (with no email).
name = "..."
email = "..."

# By default `cargo new` will initialize a new git repository. This key can be
# set to `none` to disable this behavior.
vcs = "none"

# For the following sections, $triple refers to any valid target triple, not the
# literal string "$triple", and it will apply whenever that target triple is
# being compiled to.
[target]

# For cargo builds which do not mention --target, these are the ar/linker which
# are passed to rustc to use (via `-C ar=` and `-C linker=`). By default these
# flags are not passed to the compiler.
ar = ".."
linker = ".."

[target.$triple]
# Similar to the above ar/linker configuration, but this only applies to when
# the `$triple` is being compiled for.
ar = ".."
linker = ".."


# Configuration keys related to the registry
[registry]
index = "..."   # URL of the registry index (defaults to the central repository)
token = "..."   # Access token (found on the central repo's website)

[http]
proxy = "..."     # HTTP proxy to use for HTTP requests (defaults to none)
timeout = 60000   # Timeout for each HTTP request, in milliseconds

[build]
jobs = 1        # number of jobs to run by default (default to # cpus)
```
