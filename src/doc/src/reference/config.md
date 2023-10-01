# Configuration

This document explains how Cargo’s configuration system works, as well as
available keys or configuration. For configuration of a package through its
manifest, see the [manifest format](manifest.md).

## Hierarchical structure

Cargo allows local configuration for a particular package as well as global
configuration. It looks for configuration files in the current directory and
all parent directories. If, for example, Cargo were invoked in
`/projects/foo/bar/baz`, then the following configuration files would be
probed for and unified in this order:

* `/projects/foo/bar/baz/.cargo/config.toml`
* `/projects/foo/bar/.cargo/config.toml`
* `/projects/foo/.cargo/config.toml`
* `/projects/.cargo/config.toml`
* `/.cargo/config.toml`
* `$CARGO_HOME/config.toml` which defaults to:
    * Windows: `%USERPROFILE%\.cargo\config.toml`
    * Unix: `$HOME/.cargo/config.toml`

With this structure, you can specify configuration per-package, and even
possibly check it into version control. You can also specify personal defaults
with a configuration file in your home directory.

If a key is specified in multiple config files, the values will get merged
together. Numbers, strings, and booleans will use the value in the deeper
config directory taking precedence over ancestor directories, where the
home directory is the lowest priority. Arrays will be joined together
with higher precedence items being placed later in the merged array.

At present, when being invoked from a workspace, Cargo does not read config
files from crates within the workspace. i.e. if a workspace has two crates in
it, named `/projects/foo/bar/baz/mylib` and `/projects/foo/bar/baz/mybin`, and
there are Cargo configs at `/projects/foo/bar/baz/mylib/.cargo/config.toml`
and `/projects/foo/bar/baz/mybin/.cargo/config.toml`, Cargo does not read
those configuration files if it is invoked from the workspace root
(`/projects/foo/bar/baz/`).

> **Note:** Cargo also reads config files without the `.toml` extension, such as
> `.cargo/config`. Support for the `.toml` extension was added in version 1.39
> and is the preferred form. If both files exist, Cargo will use the file
> without the extension.

## Configuration format

Configuration files are written in the [TOML format][toml] (like the
manifest), with simple key-value pairs inside of sections (tables). The
following is a quick overview of all settings, with detailed descriptions
found below.

```toml
paths = ["/path/to/override"] # path dependency overrides

[alias]     # command aliases
b = "build"
c = "check"
t = "test"
r = "run"
rr = "run --release"
recursive_example = "rr --example recursions"
space_example = ["run", "--release", "--", "\"command list\""]

[build]
jobs = 1                      # number of parallel jobs, defaults to # of CPUs
rustc = "rustc"               # the rust compiler tool
rustc-wrapper = "…"           # run this wrapper instead of `rustc`
rustc-workspace-wrapper = "…" # run this wrapper instead of `rustc` for workspace members
rustdoc = "rustdoc"           # the doc generator tool
target = "triple"             # build for the target triple (ignored by `cargo install`)
target-dir = "target"         # path of where to place all generated artifacts
rustflags = ["…", "…"]        # custom flags to pass to all compiler invocations
rustdocflags = ["…", "…"]     # custom flags to pass to rustdoc
incremental = true            # whether or not to enable incremental compilation
dep-info-basedir = "…"        # path for the base directory for targets in depfiles

[doc]
browser = "chromium"          # browser to use with `cargo doc --open`,
                              # overrides the `BROWSER` environment variable

[env]
# Set ENV_VAR_NAME=value for any process run by Cargo
ENV_VAR_NAME = "value"
# Set even if already present in environment
ENV_VAR_NAME_2 = { value = "value", force = true }
# Value is relative to .cargo directory containing `config.toml`, make absolute
ENV_VAR_NAME_3 = { value = "relative/path", relative = true }

[future-incompat-report]
frequency = 'always' # when to display a notification about a future incompat report

[cargo-new]
vcs = "none"              # VCS to use ('git', 'hg', 'pijul', 'fossil', 'none')

[http]
debug = false               # HTTP debugging
proxy = "host:port"         # HTTP proxy in libcurl format
ssl-version = "tlsv1.3"     # TLS version to use
ssl-version.max = "tlsv1.3" # maximum TLS version
ssl-version.min = "tlsv1.1" # minimum TLS version
timeout = 30                # timeout for each HTTP request, in seconds
low-speed-limit = 10        # network timeout threshold (bytes/sec)
cainfo = "cert.pem"         # path to Certificate Authority (CA) bundle
check-revoke = true         # check for SSL certificate revocation
multiplexing = true         # HTTP/2 multiplexing
user-agent = "…"            # the user-agent header

[install]
root = "/some/path"         # `cargo install` destination directory

[net]
retry = 3                   # network retries
git-fetch-with-cli = true   # use the `git` executable for git operations
offline = true              # do not access the network

[net.ssh]
known-hosts = ["..."]       # known SSH host keys

[patch.<registry>]
# Same keys as for [patch] in Cargo.toml

[profile.<name>]         # Modify profile settings via config.
inherits = "dev"         # Inherits settings from [profile.dev].
opt-level = 0            # Optimization level.
debug = true             # Include debug info.
split-debuginfo = '...'  # Debug info splitting behavior.
strip = "none"           # Removes symbols or debuginfo.
debug-assertions = true  # Enables debug assertions.
overflow-checks = true   # Enables runtime integer overflow checks.
lto = false              # Sets link-time optimization.
panic = 'unwind'         # The panic strategy.
incremental = true       # Incremental compilation.
codegen-units = 16       # Number of code generation units.
rpath = false            # Sets the rpath linking option.
[profile.<name>.build-override]  # Overrides build-script settings.
# Same keys for a normal profile.
[profile.<name>.package.<name>]  # Override profile for a package.
# Same keys for a normal profile (minus `panic`, `lto`, and `rpath`).

[registries.<name>]  # registries other than crates.io
index = "…"          # URL of the registry index
token = "…"          # authentication token for the registry

[registry]
default = "…"        # name of the default registry
token = "…"          # authentication token for crates.io

[source.<name>]      # source definition and replacement
replace-with = "…"   # replace this source with the given named source
directory = "…"      # path to a directory source
registry = "…"       # URL to a registry source
local-registry = "…" # path to a local registry source
git = "…"            # URL of a git repository source
branch = "…"         # branch name for the git repository
tag = "…"            # tag name for the git repository
rev = "…"            # revision for the git repository

[target.<triple>]
linker = "…"            # linker to use
runner = "…"            # wrapper to run executables
rustflags = ["…", "…"]  # custom flags for `rustc`

[target.<cfg>]
runner = "…"            # wrapper to run executables
rustflags = ["…", "…"]  # custom flags for `rustc`

[target.<triple>.<links>] # `links` build script override
rustc-link-lib = ["foo"]
rustc-link-search = ["/path/to/foo"]
rustc-flags = ["-L", "/some/path"]
rustc-cfg = ['key="value"']
rustc-env = {key = "value"}
rustc-cdylib-link-arg = ["…"]
metadata_key1 = "value"
metadata_key2 = "value"

[term]
quiet = false          # whether cargo output is quiet
verbose = false        # whether cargo provides verbose output
color = 'auto'         # whether cargo colorizes output
progress.when = 'auto' # whether cargo shows progress bar
progress.width = 80    # width of progress bar
```

## Environment variables

Cargo can also be configured through environment variables in addition to the
TOML configuration files. For each configuration key of the form `foo.bar` the
environment variable `CARGO_FOO_BAR` can also be used to define the value.
Keys are converted to uppercase, dots and dashes are converted to underscores.
For example the `target.x86_64-unknown-linux-gnu.runner` key can also be
defined by the `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER` environment
variable.

Environment variables will take precedence over TOML configuration files.
Currently only integer, boolean, string and some array values are supported to
be defined by environment variables. [Descriptions below](#configuration-keys)
indicate which keys support environment variables and otherwise they are not
supported due to [technical issues](https://github.com/rust-lang/cargo/issues/5416).

In addition to the system above, Cargo recognizes a few other specific
[environment variables][env].

## Command-line overrides

Cargo also accepts arbitrary configuration overrides through the
`--config` command-line option. The argument should be in TOML syntax of
`KEY=VALUE`:

```console
cargo --config net.git-fetch-with-cli=true fetch
```

The `--config` option may be specified multiple times, in which case the
values are merged in left-to-right order, using the same merging logic
that is used when multiple configuration files apply. Configuration
values specified this way take precedence over environment variables,
which take precedence over configuration files.

Some examples of what it looks like using Bourne shell syntax:

```console
# Most shells will require escaping.
cargo --config http.proxy=\"http://example.com\" …

# Spaces may be used.
cargo --config "net.git-fetch-with-cli = true" …

# TOML array example. Single quotes make it easier to read and write.
cargo --config 'build.rustdocflags = ["--html-in-header", "header.html"]' …

# Example of a complex TOML key.
cargo --config "target.'cfg(all(target_arch = \"arm\", target_os = \"none\"))'.runner = 'my-runner'" …

# Example of overriding a profile setting.
cargo --config profile.dev.package.image.opt-level=3 …
```

The `--config` option can also be used to pass paths to extra
configuration files that Cargo should use for a specific invocation.
Options from configuration files loaded this way follow the same
precedence rules as other options specified directly with `--config`.

## Config-relative paths

Paths in config files may be absolute, relative, or a bare name without any path separators.
Paths for executables without a path separator will use the `PATH` environment variable to search for the executable.
Paths for non-executables will be relative to where the config value is defined.

In particular, rules are:

* For environment variables, paths are relative to the current working directory.
* For config values loaded directly from the [`--config KEY=VALUE`](#command-line-overrides) option,
  paths are relative to the current working directory.
* For config files, paths are relative to the parent directory of the directory where the config files were defined,
  no matter those files are from either the [hierarchical probing](#hierarchical-structure)
  or the [`--config <path>`](#command-line-overrides) option.

> **Note:** To maintain consistency with existing `.cargo/config.toml` probing behavior,
> it is by design that a path in a config file passed via `--config <path>`
> is also relative to two levels up from the config file itself.
>
> To avoid unexpected results, the rule of thumb is putting your extra config files
> at the same level of discovered `.cargo/config.toml` in your project.
> For instance, given a project `/my/project`,
> it is recommended to put config files under `/my/project/.cargo`
> or a new directory at the same level, such as `/my/project/.config`.

```toml
# Relative path examples.

[target.x86_64-unknown-linux-gnu]
runner = "foo"  # Searches `PATH` for `foo`.

[source.vendored-sources]
# Directory is relative to the parent where `.cargo/config.toml` is located.
# For example, `/my/project/.cargo/config.toml` would result in `/my/project/vendor`.
directory = "vendor"
```

## Executable paths with arguments

Some Cargo commands invoke external programs, which can be configured as a path
and some number of arguments.

The value may be an array of strings like `['/path/to/program', 'somearg']` or
a space-separated string like `'/path/to/program somearg'`. If the path to the
executable contains a space, the list form must be used.

If Cargo is passing other arguments to the program such as a path to open or
run, they will be passed after the last specified argument in the value of an
option of this format. If the specified program does not have path separators,
Cargo will search `PATH` for its executable.

## Credentials

Configuration values with sensitive information are stored in the
`$CARGO_HOME/credentials.toml` file. This file is automatically created and updated
by [`cargo login`] and [`cargo logout`] when using the `cargo:token` credential provider.

It follows the same format as Cargo config files.

```toml
[registry]
token = "…"   # Access token for crates.io

[registries.<name>]
token = "…"   # Access token for the named registry
```

Tokens are used by some Cargo commands such as [`cargo publish`] for
authenticating with remote registries. Care should be taken to protect the
tokens and to keep them secret.

As with most other config values, tokens may be specified with environment
variables. The token for [crates.io] may be specified with the
`CARGO_REGISTRY_TOKEN` environment variable. Tokens for other registries may
be specified with environment variables of the form
`CARGO_REGISTRIES_<name>_TOKEN` where `<name>` is the name of the registry in
all capital letters.

> **Note:** Cargo also reads and writes credential files without the `.toml`
> extension, such as `.cargo/credentials`. Support for the `.toml` extension
> was added in version 1.39. In version 1.68, Cargo writes to the file with the
> extension by default. However, for backward compatibility reason, when both
> files exist, Cargo will read and write the file without the extension.

## Configuration keys

This section documents all configuration keys. The description for keys with
variable parts are annotated with angled brackets like `target.<triple>` where
the `<triple>` part can be any [target triple] like
`target.x86_64-pc-windows-msvc`.

### `paths`
* Type: array of strings (paths)
* Default: none
* Environment: not supported

An array of paths to local packages which are to be used as overrides for
dependencies. For more information see the [Overriding Dependencies
guide](overriding-dependencies.md#paths-overrides).

### `[alias]`
* Type: string or array of strings
* Default: see below
* Environment: `CARGO_ALIAS_<name>`

The `[alias]` table defines CLI command aliases. For example, running `cargo
b` is an alias for running `cargo build`. Each key in the table is the
subcommand, and the value is the actual command to run. The value may be an
array of strings, where the first element is the command and the following are
arguments. It may also be a string, which will be split on spaces into
subcommand and arguments. The following aliases are built-in to Cargo:

```toml
[alias]
b = "build"
c = "check"
d = "doc"
t = "test"
r = "run"
rm = "remove"
```

Aliases are not allowed to redefine existing built-in commands.

Aliases are recursive:

```toml
[alias]
rr = "run --release"
recursive_example = "rr --example recursions"
```

### `[build]`

The `[build]` table controls build-time operations and compiler settings.

#### `build.jobs`
* Type: integer or string
* Default: number of logical CPUs
* Environment: `CARGO_BUILD_JOBS`

Sets the maximum number of compiler processes to run in parallel. If negative,
it sets the maximum number of compiler processes to the number of logical CPUs
plus provided value. Should not be 0. If a string `default` is provided, it sets
the value back to defaults.

Can be overridden with the `--jobs` CLI option.

#### `build.rustc`
* Type: string (program path)
* Default: "rustc"
* Environment: `CARGO_BUILD_RUSTC` or `RUSTC`

Sets the executable to use for `rustc`.

#### `build.rustc-wrapper`
* Type: string (program path)
* Default: none
* Environment: `CARGO_BUILD_RUSTC_WRAPPER` or `RUSTC_WRAPPER`

Sets a wrapper to execute instead of `rustc`. The first argument passed to the
wrapper is the path to the actual executable to use
(i.e., `build.rustc`, if that is set, or `"rustc"` otherwise).

#### `build.rustc-workspace-wrapper`
* Type: string (program path)
* Default: none
* Environment: `CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER` or `RUSTC_WORKSPACE_WRAPPER`

Sets a wrapper to execute instead of `rustc`, for workspace members only.
The first argument passed to the wrapper is the path to the actual
executable to use (i.e., `build.rustc`, if that is set, or `"rustc"` otherwise).
It affects the filename hash so that artifacts produced by the wrapper are cached separately.

#### `build.rustdoc`
* Type: string (program path)
* Default: "rustdoc"
* Environment: `CARGO_BUILD_RUSTDOC` or `RUSTDOC`

Sets the executable to use for `rustdoc`.

#### `build.target`
* Type: string or array of strings
* Default: host platform
* Environment: `CARGO_BUILD_TARGET`

The default [target platform triples][target triple] to compile to.

This allows passing either a string or an array of strings. Each string value
is a target platform triple. The selected build targets will be built for each
of the selected architectures.

The string value may also be a relative path to a `.json` target spec file.

Can be overridden with the `--target` CLI option.

```toml
[build]
target = ["x86_64-unknown-linux-gnu", "i686-unknown-linux-gnu"]
```

#### `build.target-dir`
* Type: string (path)
* Default: "target"
* Environment: `CARGO_BUILD_TARGET_DIR` or `CARGO_TARGET_DIR`

The path to where all compiler output is placed. The default if not specified
is a directory named `target` located at the root of the workspace.

Can be overridden with the `--target-dir` CLI option.

#### `build.rustflags`
* Type: string or array of strings
* Default: none
* Environment: `CARGO_BUILD_RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` or `RUSTFLAGS`

Extra command-line flags to pass to `rustc`. The value may be an array of
strings or a space-separated string.

There are four mutually exclusive sources of extra flags. They are checked in
order, with the first one being used:

1. `CARGO_ENCODED_RUSTFLAGS` environment variable.
2. `RUSTFLAGS` environment variable.
3. All matching `target.<triple>.rustflags` and `target.<cfg>.rustflags`
   config entries joined together.
4. `build.rustflags` config value.

Additional flags may also be passed with the [`cargo rustc`] command.

If the `--target` flag (or [`build.target`](#buildtarget)) is used, then the
flags will only be passed to the compiler for the target. Things being built
for the host, such as build scripts or proc macros, will not receive the args.
Without `--target`, the flags will be passed to all compiler invocations
(including build scripts and proc macros) because dependencies are shared. If
you have args that you do not want to pass to build scripts or proc macros and
are building for the host, pass `--target` with the [host triple][target triple].

It is not recommended to pass in flags that Cargo itself usually manages. For
example, the flags driven by [profiles](profiles.md) are best handled by setting the
appropriate profile setting.

> **Caution**: Due to the low-level nature of passing flags directly to the
> compiler, this may cause a conflict with future versions of Cargo which may
> issue the same or similar flags on its own which may interfere with the
> flags you specify. This is an area where Cargo may not always be backwards
> compatible.

#### `build.rustdocflags`
* Type: string or array of strings
* Default: none
* Environment: `CARGO_BUILD_RUSTDOCFLAGS` or `CARGO_ENCODED_RUSTDOCFLAGS` or `RUSTDOCFLAGS`

Extra command-line flags to pass to `rustdoc`. The value may be an array of
strings or a space-separated string.

There are three mutually exclusive sources of extra flags. They are checked in
order, with the first one being used:

1. `CARGO_ENCODED_RUSTDOCFLAGS` environment variable.
2. `RUSTDOCFLAGS` environment variable.
3. `build.rustdocflags` config value.

Additional flags may also be passed with the [`cargo rustdoc`] command.

#### `build.incremental`
* Type: bool
* Default: from profile
* Environment: `CARGO_BUILD_INCREMENTAL` or `CARGO_INCREMENTAL`

Whether or not to perform [incremental compilation]. The default if not set is
to use the value from the [profile](profiles.md#incremental). Otherwise this overrides the setting of
all profiles.

The `CARGO_INCREMENTAL` environment variable can be set to `1` to force enable
incremental compilation for all profiles, or `0` to disable it. This env var
overrides the config setting.

#### `build.dep-info-basedir`
* Type: string (path)
* Default: none
* Environment: `CARGO_BUILD_DEP_INFO_BASEDIR`

Strips the given path prefix from [dep
info](../guide/build-cache.md#dep-info-files) file paths. This config setting
is intended to convert absolute paths to relative paths for tools that require
relative paths.

The setting itself is a config-relative path. So, for example, a value of
`"."` would strip all paths starting with the parent directory of the `.cargo`
directory.

#### `build.pipelining`

This option is deprecated and unused. Cargo always has pipelining enabled.

### `[credential-alias]`
* Type: string or array of strings
* Default: empty
* Environment: `CARGO_CREDENTIAL_ALIAS_<name>`

The `[credential-alias]` table defines credential provider aliases.
These aliases can be referenced as an element of the `registry.global-credential-providers`
array, or as a credential provider for a specific registry
under `registries.<NAME>.credential-provider`.

If specified as a string, the value will be split on spaces into path and arguments.

For example, to define an alias called `my-alias`:

```toml
[credential-alias]
my-alias = ["/usr/bin/cargo-credential-example", "--argument", "value", "--flag"]
```
See [Registry Authentication](registry-authentication.md) for more information.

### `[doc]`

The `[doc]` table defines options for the [`cargo doc`] command.

#### `doc.browser`

* Type: string or array of strings ([program path with args])
* Default: `BROWSER` environment variable, or, if that is missing,
  opening the link in a system specific way

This option sets the browser to be used by [`cargo doc`], overriding the
`BROWSER` environment variable when opening documentation with the `--open`
option.

### `[cargo-new]`

The `[cargo-new]` table defines defaults for the [`cargo new`] command.

#### `cargo-new.name`

This option is deprecated and unused.

#### `cargo-new.email`

This option is deprecated and unused.

#### `cargo-new.vcs`
* Type: string
* Default: "git" or "none"
* Environment: `CARGO_CARGO_NEW_VCS`

Specifies the source control system to use for initializing a new repository.
Valid values are `git`, `hg` (for Mercurial), `pijul`, `fossil` or `none` to
disable this behavior. Defaults to `git`, or `none` if already inside a VCS
repository. Can be overridden with the `--vcs` CLI option.

### `[env]`

The `[env]` section allows you to set additional environment variables for
build scripts, rustc invocations, `cargo run` and `cargo build`.

```toml
[env]
OPENSSL_DIR = "/opt/openssl"
```

By default, the variables specified will not override values that already exist
in the environment. This behavior can be changed by setting the `force` flag.

Setting the `relative` flag evaluates the value as a config-relative path that
is relative to the parent directory of the `.cargo` directory that contains the
`config.toml` file. The value of the environment variable will be the full
absolute path.

```toml
[env]
TMPDIR = { value = "/home/tmp", force = true }
OPENSSL_DIR = { value = "vendor/openssl", relative = true }
```

### `[future-incompat-report]`

The `[future-incompat-report]` table controls setting for [future incompat reporting](future-incompat-report.md)

#### `future-incompat-report.frequency`
* Type: string
* Default: "always"
* Environment: `CARGO_FUTURE_INCOMPAT_REPORT_FREQUENCY`

Controls how often we display a notification to the terminal when a future incompat report is available. Possible values:

* `always` (default): Always display a notification when a command (e.g. `cargo build`) produces a future incompat report
* `never`: Never display a notification

### `[http]`

The `[http]` table defines settings for HTTP behavior. This includes fetching
crate dependencies and accessing remote git repositories.

#### `http.debug`
* Type: boolean
* Default: false
* Environment: `CARGO_HTTP_DEBUG`

If `true`, enables debugging of HTTP requests. The debug information can be
seen by setting the `CARGO_LOG=network=debug` environment
variable (or use `network=trace` for even more information).

Be wary when posting logs from this output in a public location. The output
may include headers with authentication tokens which you don't want to leak!
Be sure to review logs before posting them.

#### `http.proxy`
* Type: string
* Default: none
* Environment: `CARGO_HTTP_PROXY` or `HTTPS_PROXY` or `https_proxy` or `http_proxy`

Sets an HTTP and HTTPS proxy to use. The format is in [libcurl format] as in
`[protocol://]host[:port]`. If not set, Cargo will also check the `http.proxy`
setting in your global git configuration. If none of those are set, the
`HTTPS_PROXY` or `https_proxy` environment variables set the proxy for HTTPS
requests, and `http_proxy` sets it for HTTP requests.

#### `http.timeout`
* Type: integer
* Default: 30
* Environment: `CARGO_HTTP_TIMEOUT` or `HTTP_TIMEOUT`

Sets the timeout for each HTTP request, in seconds.

#### `http.cainfo`
* Type: string (path)
* Default: none
* Environment: `CARGO_HTTP_CAINFO`

Path to a Certificate Authority (CA) bundle file, used to verify TLS
certificates. If not specified, Cargo attempts to use the system certificates.

#### `http.check-revoke`
* Type: boolean
* Default: true (Windows) false (all others)
* Environment: `CARGO_HTTP_CHECK_REVOKE`

This determines whether or not TLS certificate revocation checks should be
performed. This only works on Windows.

#### `http.ssl-version`
* Type: string or min/max table
* Default: none
* Environment: `CARGO_HTTP_SSL_VERSION`

This sets the minimum TLS version to use. It takes a string, with one of the
possible values of "default", "tlsv1", "tlsv1.0", "tlsv1.1", "tlsv1.2", or
"tlsv1.3".

This may alternatively take a table with two keys, `min` and `max`, which each
take a string value of the same kind that specifies the minimum and maximum
range of TLS versions to use.

The default is a minimum version of "tlsv1.0" and a max of the newest version
supported on your platform, typically "tlsv1.3".

#### `http.low-speed-limit`
* Type: integer
* Default: 10
* Environment: `CARGO_HTTP_LOW_SPEED_LIMIT`

This setting controls timeout behavior for slow connections. If the average
transfer speed in bytes per second is below the given value for
[`http.timeout`](#httptimeout) seconds (default 30 seconds), then the
connection is considered too slow and Cargo will abort and retry.

#### `http.multiplexing`
* Type: boolean
* Default: true
* Environment: `CARGO_HTTP_MULTIPLEXING`

When `true`, Cargo will attempt to use the HTTP2 protocol with multiplexing.
This allows multiple requests to use the same connection, usually improving
performance when fetching multiple files. If `false`, Cargo will use HTTP 1.1
without pipelining.

#### `http.user-agent`
* Type: string
* Default: Cargo's version
* Environment: `CARGO_HTTP_USER_AGENT`

Specifies a custom user-agent header to use. The default if not specified is a
string that includes Cargo's version.

### `[install]`

The `[install]` table defines defaults for the [`cargo install`] command.

#### `install.root`
* Type: string (path)
* Default: Cargo's home directory
* Environment: `CARGO_INSTALL_ROOT`

Sets the path to the root directory for installing executables for [`cargo
install`]. Executables go into a `bin` directory underneath the root.

To track information of installed executables, some extra files, such as
`.crates.toml` and `.crates2.json`, are also created under this root.

The default if not specified is Cargo's home directory (default `.cargo` in
your home directory).

Can be overridden with the `--root` command-line option.

### `[net]`

The `[net]` table controls networking configuration.

#### `net.retry`
* Type: integer
* Default: 3
* Environment: `CARGO_NET_RETRY`

Number of times to retry possibly spurious network errors.

#### `net.git-fetch-with-cli`
* Type: boolean
* Default: false
* Environment: `CARGO_NET_GIT_FETCH_WITH_CLI`

If this is `true`, then Cargo will use the `git` executable to fetch registry
indexes and git dependencies. If `false`, then it uses a built-in `git`
library.

Setting this to `true` can be helpful if you have special authentication
requirements that Cargo does not support. See [Git
Authentication](../appendix/git-authentication.md) for more information about
setting up git authentication.

#### `net.offline`
* Type: boolean
* Default: false
* Environment: `CARGO_NET_OFFLINE`

If this is `true`, then Cargo will avoid accessing the network, and attempt to
proceed with locally cached data. If `false`, Cargo will access the network as
needed, and generate an error if it encounters a network error.

Can be overridden with the `--offline` command-line option.

#### `net.ssh`

The `[net.ssh]` table contains settings for SSH connections.

#### `net.ssh.known-hosts`
* Type: array of strings
* Default: see description
* Environment: not supported

The `known-hosts` array contains a list of SSH host keys that should be
accepted as valid when connecting to an SSH server (such as for SSH git
dependencies). Each entry should be a string in a format similar to OpenSSH
`known_hosts` files. Each string should start with one or more hostnames
separated by commas, a space, the key type name, a space, and the
base64-encoded key. For example:

```toml
[net.ssh]
known-hosts = [
    "example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFO4Q5T0UV0SQevair9PFwoxY9dl4pQl3u5phoqJH3cF"
]
```

Cargo will attempt to load known hosts keys from common locations supported in
OpenSSH, and will join those with any listed in a Cargo configuration file.
If any matching entry has the correct key, the connection will be allowed.

Cargo comes with the host keys for [github.com][github-keys] built-in. If
those ever change, you can add the new keys to the config or known_hosts file.

See [Git Authentication](../appendix/git-authentication.md#ssh-known-hosts)
for more details.

[github-keys]: https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints

### `[patch]`

Just as you can override dependencies using [`[patch]` in
`Cargo.toml`](overriding-dependencies.md#the-patch-section), you can
override them in the cargo configuration file to apply those patches to
any affected build. The format is identical to the one used in
`Cargo.toml`.

Since `.cargo/config.toml` files are not usually checked into source
control, you should prefer patching using `Cargo.toml` where possible to
ensure that other developers can compile your crate in their own
environments. Patching through cargo configuration files is generally
only appropriate when the patch section is automatically generated by an
external build tool.

If a given dependency is patched both in a cargo configuration file and
a `Cargo.toml` file, the patch in the configuration file is used. If
multiple configuration files patch the same dependency, standard cargo
configuration merging is used, which prefers the value defined closest
to the current directory, with `$HOME/.cargo/config.toml` taking the
lowest precedence.

Relative `path` dependencies in such a `[patch]` section are resolved
relative to the configuration file they appear in.

### `[profile]`

The `[profile]` table can be used to globally change profile settings, and
override settings specified in `Cargo.toml`. It has the same syntax and
options as profiles specified in `Cargo.toml`. See the [Profiles chapter] for
details about the options.

[Profiles chapter]: profiles.md

#### `[profile.<name>.build-override]`
* Environment: `CARGO_PROFILE_<name>_BUILD_OVERRIDE_<key>`

The build-override table overrides settings for build scripts, proc macros,
and their dependencies. It has the same keys as a normal profile. See the
[overrides section](profiles.md#overrides) for more details.

#### `[profile.<name>.package.<name>]`
* Environment: not supported

The package table overrides settings for specific packages. It has the same
keys as a normal profile, minus the `panic`, `lto`, and `rpath` settings. See
the [overrides section](profiles.md#overrides) for more details.

#### `profile.<name>.codegen-units`
* Type: integer
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_CODEGEN_UNITS`

See [codegen-units](profiles.md#codegen-units).

#### `profile.<name>.debug`
* Type: integer or boolean
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_DEBUG`

See [debug](profiles.md#debug).

#### `profile.<name>.split-debuginfo`
* Type: string
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_SPLIT_DEBUGINFO`

See [split-debuginfo](profiles.md#split-debuginfo).

#### `profile.<name>.strip`
* Type: string or boolean
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_STRIP`

See [strip](profiles.md#strip).

#### `profile.<name>.debug-assertions`
* Type: boolean
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_DEBUG_ASSERTIONS`

See [debug-assertions](profiles.md#debug-assertions).

#### `profile.<name>.incremental`
* Type: boolean
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_INCREMENTAL`

See [incremental](profiles.md#incremental).

#### `profile.<name>.lto`
* Type: string or boolean
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_LTO`

See [lto](profiles.md#lto).

#### `profile.<name>.overflow-checks`
* Type: boolean
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_OVERFLOW_CHECKS`

See [overflow-checks](profiles.md#overflow-checks).

#### `profile.<name>.opt-level`
* Type: integer or string
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_OPT_LEVEL`

See [opt-level](profiles.md#opt-level).

#### `profile.<name>.panic`
* Type: string
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_PANIC`

See [panic](profiles.md#panic).

#### `profile.<name>.rpath`
* Type: boolean
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_RPATH`

See [rpath](profiles.md#rpath).

#### `profile.<name>.strip`
* Type: string
* Default: See profile docs.
* Environment: `CARGO_PROFILE_<name>_STRIP`

See [strip](profiles.md#strip).


### `[registries]`

The `[registries]` table is used for specifying additional [registries]. It
consists of a sub-table for each named registry.

#### `registries.<name>.index`
* Type: string (url)
* Default: none
* Environment: `CARGO_REGISTRIES_<name>_INDEX`

Specifies the URL of the index for the registry.

#### `registries.<name>.token`
* Type: string
* Default: none
* Environment: `CARGO_REGISTRIES_<name>_TOKEN`

Specifies the authentication token for the given registry. This value should
only appear in the [credentials](#credentials) file. This is used for registry
commands like [`cargo publish`] that require authentication.

Can be overridden with the `--token` command-line option.

#### `registries.<name>.credential-provider`
* Type: string or array of path and arguments
* Default: none
* Environment: `CARGO_REGISTRIES_<name>_CREDENTIAL_PROVIDER`

Specifies the credential provider for the given registry. If not set, the
providers in [`registry.global-credential-providers`](#registryglobal-credential-providers)
will be used.

If specified as a string, path and arguments will be split on spaces. For
paths or arguments that contain spaces, use an array.

If the value exists in the [`[credential-alias]`](#credential-alias) table, the alias will be used.

See [Registry Authentication](registry-authentication.md) for more information.

#### `registries.crates-io.protocol`
* Type: string
* Default: `sparse`
* Environment: `CARGO_REGISTRIES_CRATES_IO_PROTOCOL`

Specifies the protocol used to access crates.io. Allowed values are `git` or `sparse`.

`git` causes Cargo to clone the entire index of all packages ever published to [crates.io] from <https://github.com/rust-lang/crates.io-index/>.
This can have performance implications due to the size of the index.
`sparse` is a newer protocol which uses HTTPS to download only what is necessary from <https://index.crates.io/>.
This can result in a significant performance improvement for resolving new dependencies in most situations.

More information about registry protocols may be found in the [Registries chapter](registries.md).

### `[registry]`

The `[registry]` table controls the default registry used when one is not
specified.

#### `registry.index`

This value is no longer accepted and should not be used.

#### `registry.default`
* Type: string
* Default: `"crates-io"`
* Environment: `CARGO_REGISTRY_DEFAULT`

The name of the registry (from the [`registries` table](#registries)) to use
by default for registry commands like [`cargo publish`].

Can be overridden with the `--registry` command-line option.

#### `registry.credential-provider`
* Type: string or array of path and arguments
* Default: none
* Environment: `CARGO_REGISTRY_CREDENTIAL_PROVIDER`

Specifies the credential provider for [crates.io]. If not set, the
providers in [`registry.global-credential-providers`](#registryglobal-credential-providers)
will be used.

If specified as a string, path and arguments will be split on spaces. For
paths or arguments that contain spaces, use an array.

If the value exists in the `[credential-alias]` table, the alias will be used.

See [Registry Authentication](registry-authentication.md) for more information.

#### `registry.token`
* Type: string
* Default: none
* Environment: `CARGO_REGISTRY_TOKEN`

Specifies the authentication token for [crates.io]. This value should only
appear in the [credentials](#credentials) file. This is used for registry
commands like [`cargo publish`] that require authentication.

Can be overridden with the `--token` command-line option.

#### `registry.global-credential-providers`
* Type: array
* Default: `["cargo:token"]`
* Environment: `CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS`

Specifies the list of global credential providers. If credential provider is not set
for a specific registry using `registries.<name>.credential-provider`, Cargo will use
the credential providers in this list. Providers toward the end of the list have precedence.

Path and arguments are split on spaces. If the path or arguments contains spaces, the credential
provider should be defined in the [`[credential-alias]`](#credential-alias) table and
referenced here by its alias.

See [Registry Authentication](registry-authentication.md) for more information.

### `[source]`

The `[source]` table defines the registry sources available. See [Source
Replacement] for more information. It consists of a sub-table for each named
source. A source should only define one kind (directory, registry,
local-registry, or git).

#### `source.<name>.replace-with`
* Type: string
* Default: none
* Environment: not supported

If set, replace this source with the given named source or named registry.

#### `source.<name>.directory`
* Type: string (path)
* Default: none
* Environment: not supported

Sets the path to a directory to use as a directory source.

#### `source.<name>.registry`
* Type: string (url)
* Default: none
* Environment: not supported

Sets the URL to use for a registry source.

#### `source.<name>.local-registry`
* Type: string (path)
* Default: none
* Environment: not supported

Sets the path to a directory to use as a local registry source.

#### `source.<name>.git`
* Type: string (url)
* Default: none
* Environment: not supported

Sets the URL to use for a git repository source.

#### `source.<name>.branch`
* Type: string
* Default: none
* Environment: not supported

Sets the branch name to use for a git repository.

If none of `branch`, `tag`, or `rev` is set, defaults to the `master` branch.

#### `source.<name>.tag`
* Type: string
* Default: none
* Environment: not supported

Sets the tag name to use for a git repository.

If none of `branch`, `tag`, or `rev` is set, defaults to the `master` branch.

#### `source.<name>.rev`
* Type: string
* Default: none
* Environment: not supported

Sets the [revision] to use for a git repository.

If none of `branch`, `tag`, or `rev` is set, defaults to the `master` branch.


### `[target]`

The `[target]` table is used for specifying settings for specific platform
targets. It consists of a sub-table which is either a [platform triple][target triple] 
or a [`cfg()` expression]. The given values will be used if the target platform
matches either the `<triple>` value or the `<cfg>` expression.

```toml
[target.thumbv7m-none-eabi]
linker = "arm-none-eabi-gcc"
runner = "my-emulator"
rustflags = ["…", "…"]

[target.'cfg(all(target_arch = "arm", target_os = "none"))']
runner = "my-arm-wrapper"
rustflags = ["…", "…"]
```

`cfg` values come from those built-in to the compiler (run `rustc --print=cfg`
to view), values set by [build scripts], and extra `--cfg` flags passed to
`rustc` (such as those defined in `RUSTFLAGS`). Do not try to match on
`debug_assertions` or Cargo features like `feature="foo"`.

If using a target spec JSON file, the [`<triple>`] value is the filename stem.
For example `--target foo/bar.json` would match `[target.bar]`.

#### `target.<triple>.ar`

This option is deprecated and unused.

#### `target.<triple>.linker`
* Type: string (program path)
* Default: none
* Environment: `CARGO_TARGET_<triple>_LINKER`

Specifies the linker which is passed to `rustc` (via [`-C linker`]) when the
[`<triple>`] is being compiled for. By default, the linker is not overridden.

#### `target.<cfg>.linker`
This is similar to the [target linker](#targettriplelinker), but using
a [`cfg()` expression]. If both a [`<triple>`] and `<cfg>` runner match,
the `<triple>` will take precedence. It is an error if more than one
`<cfg>` runner matches the current target.

#### `target.<triple>.runner`
* Type: string or array of strings ([program path with args])
* Default: none
* Environment: `CARGO_TARGET_<triple>_RUNNER`

If a runner is provided, executables for the target [`<triple>`] will be
executed by invoking the specified runner with the actual executable passed as
an argument. This applies to [`cargo run`], [`cargo test`] and [`cargo bench`]
commands. By default, compiled executables are executed directly.

#### `target.<cfg>.runner`

This is similar to the [target runner](#targettriplerunner), but using
a [`cfg()` expression]. If both a [`<triple>`] and `<cfg>` runner match,
the `<triple>` will take precedence. It is an error if more than one
`<cfg>` runner matches the current target.

#### `target.<triple>.rustflags`
* Type: string or array of strings
* Default: none
* Environment: `CARGO_TARGET_<triple>_RUSTFLAGS`

Passes a set of custom flags to the compiler for this [`<triple>`]. 
The value may be an array of strings or a space-separated string.

See [`build.rustflags`](#buildrustflags) for more details on the different
ways to specific extra flags.

#### `target.<cfg>.rustflags`

This is similar to the [target rustflags](#targettriplerustflags), but
using a [`cfg()` expression]. If several `<cfg>` and [`<triple>`] entries
match the current target, the flags are joined together.

#### `target.<triple>.<links>`

The links sub-table provides a way to [override a build script]. When
specified, the build script for the given `links` library will not be
run, and the given values will be used instead.

```toml
[target.x86_64-unknown-linux-gnu.foo]
rustc-link-lib = ["foo"]
rustc-link-search = ["/path/to/foo"]
rustc-flags = "-L /some/path"
rustc-cfg = ['key="value"']
rustc-env = {key = "value"}
rustc-cdylib-link-arg = ["…"]
metadata_key1 = "value"
metadata_key2 = "value"
```

### `[term]`

The `[term]` table controls terminal output and interaction.

#### `term.quiet`
* Type: boolean
* Default: false
* Environment: `CARGO_TERM_QUIET`

Controls whether or not log messages are displayed by Cargo.

Specifying the `--quiet` flag will override and force quiet output.
Specifying the `--verbose` flag will override and disable quiet output.

#### `term.verbose`
* Type: boolean
* Default: false
* Environment: `CARGO_TERM_VERBOSE`

Controls whether or not extra detailed messages are displayed by Cargo.

Specifying the `--quiet` flag will override and disable verbose output.
Specifying the `--verbose` flag will override and force verbose output.

#### `term.color`
* Type: string
* Default: "auto"
* Environment: `CARGO_TERM_COLOR`

Controls whether or not colored output is used in the terminal. Possible values:

* `auto` (default): Automatically detect if color support is available on the
  terminal.
* `always`: Always display colors.
* `never`: Never display colors.

Can be overridden with the `--color` command-line option.

#### `term.progress.when`
* Type: string
* Default: "auto"
* Environment: `CARGO_TERM_PROGRESS_WHEN`

Controls whether or not progress bar is shown in the terminal. Possible values:

* `auto` (default): Intelligently guess whether to show progress bar.
* `always`: Always show progress bar.
* `never`: Never show progress bar.

#### `term.progress.width`
* Type: integer
* Default: none
* Environment: `CARGO_TERM_PROGRESS_WIDTH`

Sets the width for progress bar.

[`cargo bench`]: ../commands/cargo-bench.md
[`cargo login`]: ../commands/cargo-login.md
[`cargo logout`]: ../commands/cargo-logout.md
[`cargo doc`]: ../commands/cargo-doc.md
[`cargo new`]: ../commands/cargo-new.md
[`cargo publish`]: ../commands/cargo-publish.md
[`cargo run`]: ../commands/cargo-run.md
[`cargo rustc`]: ../commands/cargo-rustc.md
[`cargo test`]: ../commands/cargo-test.md
[`cargo rustdoc`]: ../commands/cargo-rustdoc.md
[`cargo install`]: ../commands/cargo-install.md
[env]: environment-variables.md
[`cfg()` expression]: ../../reference/conditional-compilation.html
[build scripts]: build-scripts.md
[`-C linker`]: ../../rustc/codegen-options/index.md#linker
[override a build script]: build-scripts.md#overriding-build-scripts
[toml]: https://toml.io/
[incremental compilation]: profiles.md#incremental
[program path with args]: #executable-paths-with-arguments
[libcurl format]: https://everything.curl.dev/libcurl/proxies#proxy-types
[source replacement]: source-replacement.md
[revision]: https://git-scm.com/docs/gitrevisions
[registries]: registries.md
[crates.io]: https://crates.io/
[target triple]: ../appendix/glossary.md#target '"target" (glossary)'
[`<triple>`]: ../appendix/glossary.md#target '"target" (glossary)'
