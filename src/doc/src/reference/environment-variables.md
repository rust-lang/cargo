# Environment Variables

Cargo sets and reads a number of environment variables which your code can detect
or override. Here is a list of the variables Cargo sets, organized by when it interacts
with them:

## Environment variables Cargo reads

You can override these environment variables to change Cargo's behavior on your
system:

* `CARGO_LOG` --- Cargo uses the [`tracing`] crate to display debug log messages.
  The `CARGO_LOG` environment variable can be set to enable debug logging,
  with a value such as `trace`, `debug`, or `warn`.
  Usually it is only used during debugging. For more details refer to the
  [Debug logging].
* `CARGO_HOME` --- Cargo maintains a local cache of the registry index and of
  git checkouts of crates. By default these are stored under `$HOME/.cargo`
  (`%USERPROFILE%\.cargo` on Windows), but this variable overrides the
  location of this directory. Once a crate is cached it is not removed by the
  clean command.
  For more details refer to the [guide](../guide/cargo-home.md).
* `CARGO_TARGET_DIR` --- Location of where to place all generated artifacts,
  relative to the current working directory. See [`build.target-dir`] to set
  via config.
* `CARGO` --- If set, Cargo will forward this value instead of setting it
  to its own auto-detected path when it builds crates and when it
  executes build scripts and external subcommands. This value is not
  directly executed by Cargo, and should always point at a command that
  behaves exactly like `cargo`, as that's what users of the variable
  will be expecting.
* `RUSTC` --- Instead of running `rustc`, Cargo will execute this specified
  compiler instead. See [`build.rustc`] to set via config.
* `RUSTC_WRAPPER` --- Instead of simply running `rustc`, Cargo will execute this
  specified wrapper, passing as its command-line arguments the rustc
  invocation, with the first argument being the path to the actual rustc.
  Useful to set up a build cache tool such as `sccache`. See
  [`build.rustc-wrapper`] to set via config. Setting this to the empty string
  overwrites the config and resets cargo to not use a wrapper.
* `RUSTC_WORKSPACE_WRAPPER` --- Instead of simply running `rustc`, for workspace
  members Cargo will execute this specified wrapper, passing
  as its command-line arguments the rustc invocation, with the first argument
  being the path to the actual rustc. It affects the filename hash
  so that artifacts produced by the wrapper are cached separately.
  See [`build.rustc-workspace-wrapper`] to set via config. Setting this to the empty string
  overwrites the config and resets cargo to not use a wrapper for workspace members.
* `RUSTDOC` --- Instead of running `rustdoc`, Cargo will execute this specified
  `rustdoc` instance instead. See [`build.rustdoc`] to set via config.
* `RUSTDOCFLAGS` --- A space-separated list of custom flags to pass to all `rustdoc`
  invocations that Cargo performs. In contrast with [`cargo rustdoc`], this is
  useful for passing a flag to *all* `rustdoc` instances. See
  [`build.rustdocflags`] for some more ways to set flags. This string is
  split by whitespace; for a more robust encoding of multiple arguments,
  see `CARGO_ENCODED_RUSTDOCFLAGS`.
* `CARGO_ENCODED_RUSTDOCFLAGS` ---  A list of custom flags separated by `0x1f`
  (ASCII Unit Separator) to pass to all `rustdoc` invocations that Cargo performs.
* `RUSTFLAGS` --- A space-separated list of custom flags to pass to all compiler
  invocations that Cargo performs. In contrast with [`cargo rustc`], this is
  useful for passing a flag to *all* compiler instances. See
  [`build.rustflags`] for some more ways to set flags. This string is
  split by whitespace; for a more robust encoding of multiple arguments,
  see `CARGO_ENCODED_RUSTFLAGS`.
* `CARGO_ENCODED_RUSTFLAGS` --- A list of custom flags separated by `0x1f`
  (ASCII Unit Separator) to pass to all compiler invocations that Cargo performs.
* `CARGO_INCREMENTAL` --- If this is set to 1 then Cargo will force [incremental
  compilation] to be enabled for the current compilation, and when set to 0 it
  will force disabling it. If this env var isn't present then cargo's defaults
  will otherwise be used. See also [`build.incremental`] config value.
* `CARGO_CACHE_RUSTC_INFO` --- If this is set to 0 then Cargo will not try to cache
  compiler version information.
* `HTTPS_PROXY` or `https_proxy` or `http_proxy` --- The HTTP proxy to use, see
  [`http.proxy`] for more detail.
* `HTTP_TIMEOUT` --- The HTTP timeout in seconds, see [`http.timeout`] for more
  detail.
* `TERM` --- If this is set to `dumb`, it disables the progress bar.
* `BROWSER` --- The web browser to execute to open documentation with [`cargo
  doc`]'s' `--open` flag, see [`doc.browser`] for more details.
* `RUSTFMT` --- Instead of running `rustfmt`,
  [`cargo fmt`](https://github.com/rust-lang/rustfmt) will execute this specified
  `rustfmt` instance instead.

### Configuration environment variables

Cargo reads environment variables for some configuration values.
See the [configuration chapter][config-env] for more details.
In summary, the supported environment variables are:

* `CARGO_ALIAS_<name>` --- Command aliases, see [`alias`].
* `CARGO_BUILD_JOBS` --- Number of parallel jobs, see [`build.jobs`].
* `CARGO_BUILD_RUSTC` --- The `rustc` executable, see [`build.rustc`].
* `CARGO_BUILD_RUSTC_WRAPPER` --- The `rustc` wrapper, see [`build.rustc-wrapper`].
* `CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER` --- The `rustc` wrapper for workspace members only, see [`build.rustc-workspace-wrapper`].
* `CARGO_BUILD_RUSTDOC` --- The `rustdoc` executable, see [`build.rustdoc`].
* `CARGO_BUILD_TARGET` --- The default target platform, see [`build.target`].
* `CARGO_BUILD_TARGET_DIR` --- The default output directory, see [`build.target-dir`].
* `CARGO_BUILD_RUSTFLAGS` --- Extra `rustc` flags, see [`build.rustflags`].
* `CARGO_BUILD_RUSTDOCFLAGS` --- Extra `rustdoc` flags, see [`build.rustdocflags`].
* `CARGO_BUILD_INCREMENTAL` --- Incremental compilation, see [`build.incremental`].
* `CARGO_BUILD_DEP_INFO_BASEDIR` --- Dep-info relative directory, see [`build.dep-info-basedir`].
* `CARGO_CARGO_NEW_VCS` --- The default source control system with [`cargo new`], see [`cargo-new.vcs`].
* `CARGO_FUTURE_INCOMPAT_REPORT_FREQUENCY` --- How often we should generate a future incompat report notification, see [`future-incompat-report.frequency`].
* `CARGO_HTTP_DEBUG` --- Enables HTTP debugging, see [`http.debug`].
* `CARGO_HTTP_PROXY` --- Enables HTTP proxy, see [`http.proxy`].
* `CARGO_HTTP_TIMEOUT` --- The HTTP timeout, see [`http.timeout`].
* `CARGO_HTTP_CAINFO` --- The TLS certificate Certificate Authority file, see [`http.cainfo`].
* `CARGO_HTTP_CHECK_REVOKE` --- Disables TLS certificate revocation checks, see [`http.check-revoke`].
* `CARGO_HTTP_SSL_VERSION` --- The TLS version to use, see [`http.ssl-version`].
* `CARGO_HTTP_LOW_SPEED_LIMIT` --- The HTTP low-speed limit, see [`http.low-speed-limit`].
* `CARGO_HTTP_MULTIPLEXING` --- Whether HTTP/2 multiplexing is used, see [`http.multiplexing`].
* `CARGO_HTTP_USER_AGENT` --- The HTTP user-agent header, see [`http.user-agent`].
* `CARGO_INSTALL_ROOT` --- The default directory for [`cargo install`], see [`install.root`].
* `CARGO_NET_RETRY` --- Number of times to retry network errors, see [`net.retry`].
* `CARGO_NET_GIT_FETCH_WITH_CLI` --- Enables the use of the `git` executable to fetch, see [`net.git-fetch-with-cli`].
* `CARGO_NET_OFFLINE` --- Offline mode, see [`net.offline`].
* `CARGO_PROFILE_<name>_BUILD_OVERRIDE_<key>` --- Override build script profile, see [`profile.<name>.build-override`].
* `CARGO_PROFILE_<name>_CODEGEN_UNITS` --- Set code generation units, see [`profile.<name>.codegen-units`].
* `CARGO_PROFILE_<name>_DEBUG` --- What kind of debug info to include, see [`profile.<name>.debug`].
* `CARGO_PROFILE_<name>_DEBUG_ASSERTIONS` --- Enable/disable debug assertions, see [`profile.<name>.debug-assertions`].
* `CARGO_PROFILE_<name>_INCREMENTAL` --- Enable/disable incremental compilation, see [`profile.<name>.incremental`].
* `CARGO_PROFILE_<name>_LTO` --- Link-time optimization, see [`profile.<name>.lto`].
* `CARGO_PROFILE_<name>_OVERFLOW_CHECKS` --- Enable/disable overflow checks, see [`profile.<name>.overflow-checks`].
* `CARGO_PROFILE_<name>_OPT_LEVEL` --- Set the optimization level, see [`profile.<name>.opt-level`].
* `CARGO_PROFILE_<name>_PANIC` --- The panic strategy to use, see [`profile.<name>.panic`].
* `CARGO_PROFILE_<name>_RPATH` --- The rpath linking option, see [`profile.<name>.rpath`].
* `CARGO_PROFILE_<name>_SPLIT_DEBUGINFO` --- Controls debug file output behavior, see [`profile.<name>.split-debuginfo`].
* `CARGO_PROFILE_<name>_STRIP` --- Controls stripping of symbols and/or debuginfos, see [`profile.<name>.strip`].
* `CARGO_REGISTRIES_<name>_CREDENTIAL_PROVIDER` --- Credential provider for a registry, see [`registries.<name>.credential-provider`].
* `CARGO_REGISTRIES_<name>_INDEX` --- URL of a registry index, see [`registries.<name>.index`].
* `CARGO_REGISTRIES_<name>_TOKEN` --- Authentication token of a registry, see [`registries.<name>.token`].
* `CARGO_REGISTRY_CREDENTIAL_PROVIDER` --- Credential provider for [crates.io], see [`registry.credential-provider`].
* `CARGO_REGISTRY_DEFAULT` --- Default registry for the `--registry` flag, see [`registry.default`].
* `CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS` --- Credential providers for registries that do not have a specific provider defined. See [`registry.global-credential-providers`].
* `CARGO_REGISTRY_TOKEN` --- Authentication token for [crates.io], see [`registry.token`].
* `CARGO_TARGET_<triple>_LINKER` --- The linker to use, see [`target.<triple>.linker`]. The triple must be [converted to uppercase and underscores](config.md#environment-variables).
* `CARGO_TARGET_<triple>_RUNNER` --- The executable runner, see [`target.<triple>.runner`].
* `CARGO_TARGET_<triple>_RUSTFLAGS` --- Extra `rustc` flags for a target, see [`target.<triple>.rustflags`].
* `CARGO_TERM_QUIET` --- Quiet mode, see [`term.quiet`].
* `CARGO_TERM_VERBOSE` --- The default terminal verbosity, see [`term.verbose`].
* `CARGO_TERM_COLOR` --- The default color mode, see [`term.color`].
* `CARGO_TERM_PROGRESS_WHEN` --- The default progress bar showing mode, see [`term.progress.when`].
* `CARGO_TERM_PROGRESS_WIDTH` --- The default progress bar width, see [`term.progress.width`].

[`cargo doc`]: ../commands/cargo-doc.md
[`cargo install`]: ../commands/cargo-install.md
[`cargo new`]: ../commands/cargo-new.md
[`cargo rustc`]: ../commands/cargo-rustc.md
[`cargo rustdoc`]: ../commands/cargo-rustdoc.md
[config-env]: config.md#environment-variables
[crates.io]: https://crates.io/
[incremental compilation]: profiles.md#incremental
[`alias`]: config.md#alias
[`build.jobs`]: config.md#buildjobs
[`build.rustc`]: config.md#buildrustc
[`build.rustc-wrapper`]: config.md#buildrustc-wrapper
[`build.rustc-workspace-wrapper`]: config.md#buildrustc-workspace-wrapper
[`build.rustdoc`]: config.md#buildrustdoc
[`build.target`]: config.md#buildtarget
[`build.target-dir`]: config.md#buildtarget-dir
[`build.rustflags`]: config.md#buildrustflags
[`build.rustdocflags`]: config.md#buildrustdocflags
[`build.incremental`]: config.md#buildincremental
[`build.dep-info-basedir`]: config.md#builddep-info-basedir
[`doc.browser`]: config.md#docbrowser
[`cargo-new.name`]: config.md#cargo-newname
[`cargo-new.email`]: config.md#cargo-newemail
[`cargo-new.vcs`]: config.md#cargo-newvcs
[`future-incompat-report.frequency`]: config.md#future-incompat-reportfrequency
[`http.debug`]: config.md#httpdebug
[`http.proxy`]: config.md#httpproxy
[`http.timeout`]: config.md#httptimeout
[`http.cainfo`]: config.md#httpcainfo
[`http.check-revoke`]: config.md#httpcheck-revoke
[`http.ssl-version`]: config.md#httpssl-version
[`http.low-speed-limit`]: config.md#httplow-speed-limit
[`http.multiplexing`]: config.md#httpmultiplexing
[`http.user-agent`]: config.md#httpuser-agent
[`install.root`]: config.md#installroot
[`net.retry`]: config.md#netretry
[`net.git-fetch-with-cli`]: config.md#netgit-fetch-with-cli
[`net.offline`]: config.md#netoffline
[`profile.<name>.build-override`]: config.md#profilenamebuild-override
[`profile.<name>.codegen-units`]: config.md#profilenamecodegen-units
[`profile.<name>.debug`]: config.md#profilenamedebug
[`profile.<name>.debug-assertions`]: config.md#profilenamedebug-assertions
[`profile.<name>.incremental`]: config.md#profilenameincremental
[`profile.<name>.lto`]: config.md#profilenamelto
[`profile.<name>.overflow-checks`]: config.md#profilenameoverflow-checks
[`profile.<name>.opt-level`]: config.md#profilenameopt-level
[`profile.<name>.panic`]: config.md#profilenamepanic
[`profile.<name>.rpath`]: config.md#profilenamerpath
[`profile.<name>.split-debuginfo`]: config.md#profilenamesplit-debuginfo
[`profile.<name>.strip`]: config.md#profilenamestrip
[`registries.<name>.credential-provider`]: config.md#registriesnamecredential-provider
[`registries.<name>.index`]: config.md#registriesnameindex
[`registries.<name>.token`]: config.md#registriesnametoken
[`registry.credential-provider`]: config.md#registrycredential-provider
[`registry.default`]: config.md#registrydefault
[`registry.global-credential-providers`]: config.md#registryglobal-credential-providers
[`registry.token`]: config.md#registrytoken
[`target.<triple>.linker`]: config.md#targettriplelinker
[`target.<triple>.runner`]: config.md#targettriplerunner
[`target.<triple>.rustflags`]: config.md#targettriplerustflags
[`term.quiet`]: config.md#termquiet
[`term.verbose`]: config.md#termverbose
[`term.color`]: config.md#termcolor
[`term.progress.when`]: config.md#termprogresswhen
[`term.progress.width`]: config.md#termprogresswidth

## Environment variables Cargo sets for crates

Cargo exposes these environment variables to your crate when it is compiled.
Note that this applies for running binaries with `cargo run` and `cargo test`
as well. To get the value of any of these variables in a Rust program, do
this:

```rust,ignore
let version = env!("CARGO_PKG_VERSION");
```

`version` will now contain the value of `CARGO_PKG_VERSION`.

Note that if one of these values is not provided in the manifest, the
corresponding environment variable is set to the empty string, `""`.

* `CARGO` --- Path to the `cargo` binary performing the build.
* `CARGO_MANIFEST_DIR` --- The directory containing the manifest of your package.
* `CARGO_PKG_VERSION` --- The full version of your package.
* `CARGO_PKG_VERSION_MAJOR` --- The major version of your package.
* `CARGO_PKG_VERSION_MINOR` --- The minor version of your package.
* `CARGO_PKG_VERSION_PATCH` --- The patch version of your package.
* `CARGO_PKG_VERSION_PRE` --- The pre-release version of your package.
* `CARGO_PKG_AUTHORS` --- Colon separated list of authors from the manifest of your package.
* `CARGO_PKG_NAME` --- The name of your package.
* `CARGO_PKG_DESCRIPTION` --- The description from the manifest of your package.
* `CARGO_PKG_HOMEPAGE` --- The home page from the manifest of your package.
* `CARGO_PKG_REPOSITORY` --- The repository from the manifest of your package.
* `CARGO_PKG_LICENSE` --- The license from the manifest of your package.
* `CARGO_PKG_LICENSE_FILE` --- The license file from the manifest of your package.
* `CARGO_PKG_RUST_VERSION` --- The Rust version from the manifest of your package.
  Note that this is the minimum Rust version supported by the package, not the
  current Rust version.
* `CARGO_PKG_README` --- Path to the README file of your package.
* `CARGO_CRATE_NAME` --- The name of the crate that is currently being compiled. It is the name of the [Cargo target] with `-` converted to `_`, such as the name of the library, binary, example, integration test, or benchmark.
* `CARGO_BIN_NAME` --- The name of the binary that is currently being compiled.
  Only set for [binaries] or binary [examples]. This name does not include any
  file extension, such as `.exe`.
* `OUT_DIR` --- If the package has a build script, this is set to the folder where the build
              script should place its output. See below for more information.
              (Only set during compilation.)
* `CARGO_BIN_EXE_<name>` --- The absolute path to a binary target's executable.
  This is only set when building an [integration test] or benchmark. This may
  be used with the [`env` macro] to find the executable to run for testing
  purposes. The `<name>` is the name of the binary target, exactly as-is. For
  example, `CARGO_BIN_EXE_my-program` for a binary named `my-program`.
  Binaries are automatically built when the test is built, unless the binary
  has required features that are not enabled.
* `CARGO_PRIMARY_PACKAGE` --- This environment variable will be set if the
  package being built is primary. Primary packages are the ones the user
  selected on the command-line, either with `-p` flags or the defaults based
  on the current directory and the default workspace members. This environment
  variable will not be set when building dependencies. This is only set when
  compiling the package (not when running binaries or tests).
* `CARGO_TARGET_TMPDIR` --- Only set when building [integration test] or benchmark code.
  This is a path to a directory inside the target directory
  where integration tests or benchmarks are free to put any data needed by
  the tests/benches. Cargo initially creates this directory but doesn't
  manage its content in any way, this is the responsibility of the test code.

[Cargo target]: cargo-targets.md
[binaries]: cargo-targets.md#binaries
[examples]: cargo-targets.md#examples
[integration test]: cargo-targets.md#integration-tests
[`env` macro]: ../../std/macro.env.html

### Dynamic library paths

Cargo also sets the dynamic library path when compiling and running binaries
with commands like `cargo run` and `cargo test`. This helps with locating
shared libraries that are part of the build process. The variable name depends
on the platform:

* Windows: `PATH`
* macOS: `DYLD_FALLBACK_LIBRARY_PATH`
* Unix: `LD_LIBRARY_PATH`
* AIX: `LIBPATH`

The value is extended from the existing value when Cargo starts. macOS has
special consideration where if `DYLD_FALLBACK_LIBRARY_PATH` is not already
set, it will add the default `$HOME/lib:/usr/local/lib:/usr/lib`.

Cargo includes the following paths:

* Search paths included from any build script with the [`rustc-link-search`
  instruction](build-scripts.md#rustc-link-search). Paths outside of the
  `target` directory are removed. It is the responsibility of the user running
  Cargo to properly set the environment if additional libraries on the system
  are needed in the search path.
* The base output directory, such as `target/debug`, and the "deps" directory.
  This is mostly for legacy support of `rustc` compiler plugins.
* The rustc sysroot library path. This generally is not important to most
  users.

## Environment variables Cargo sets for build scripts

Cargo sets several environment variables when build scripts are run. Because these variables
are not yet set when the build script is compiled, the above example using `env!` won't work
and instead you'll need to retrieve the values when the build script is run:

```rust,ignore
use std::env;
let out_dir = env::var("OUT_DIR").unwrap();
```

`out_dir` will now contain the value of `OUT_DIR`.

* `CARGO` --- Path to the `cargo` binary performing the build.
* `CARGO_MANIFEST_DIR` --- The directory containing the manifest for the package
                         being built (the package containing the build
                         script). Also note that this is the value of the
                         current working directory of the build script when it
                         starts.
* `CARGO_MANIFEST_LINKS` --- the manifest `links` value.
* `CARGO_MAKEFLAGS` --- Contains parameters needed for Cargo's [jobserver]
                      implementation to parallelize subprocesses.
                      Rustc or cargo invocations from build.rs can already
                      read `CARGO_MAKEFLAGS`, but GNU Make requires the
                      flags to be specified either directly as arguments,
                      or through the `MAKEFLAGS` environment variable.
                      Currently Cargo doesn't set the `MAKEFLAGS` variable,
                      but it's free for build scripts invoking GNU Make
                      to set it to the contents of `CARGO_MAKEFLAGS`.
* `CARGO_FEATURE_<name>` --- For each activated feature of the package being
                           built, this environment variable will be present
                           where `<name>` is the name of the feature uppercased
                           and having `-` translated to `_`.
* `CARGO_CFG_<cfg>` --- For each [configuration option][configuration] of the
  package being built, this environment variable will contain the value of the
  configuration, where `<cfg>` is the name of the configuration uppercased and
  having `-` translated to `_`. Boolean configurations are present if they are
  set, and not present otherwise. Configurations with multiple values are
  joined to a single variable with the values delimited by `,`. This includes
  values built-in to the compiler (which can be seen with `rustc --print=cfg`)
  and values set by build scripts and extra flags passed to `rustc` (such as
  those defined in `RUSTFLAGS`). Some examples of what these variables are:
    * `CARGO_CFG_UNIX` --- Set on [unix-like platforms].
    * `CARGO_CFG_WINDOWS` --- Set on [windows-like platforms].
    * `CARGO_CFG_TARGET_FAMILY=unix` --- The [target family].
    * `CARGO_CFG_TARGET_OS=macos` --- The [target operating system].
    * `CARGO_CFG_TARGET_ARCH=x86_64` --- The CPU [target architecture].
    * `CARGO_CFG_TARGET_VENDOR=apple` --- The [target vendor].
    * `CARGO_CFG_TARGET_ENV=gnu` --- The [target environment] ABI.
    * `CARGO_CFG_TARGET_POINTER_WIDTH=64` --- The CPU [pointer width].
    * `CARGO_CFG_TARGET_ENDIAN=little` --- The CPU [target endianness].
    * `CARGO_CFG_TARGET_FEATURE=mmx,sse` --- List of CPU [target features] enabled.
* `OUT_DIR` --- the folder in which all output and intermediate artifacts should
              be placed. This folder is inside the build directory for the
              package being built, and it is unique for the package in question.
* `TARGET` --- the target triple that is being compiled for. Native code should be
             compiled for this triple. See the [Target Triple] description
             for more information.
* `HOST` --- the host triple of the Rust compiler.
* `NUM_JOBS` --- the parallelism specified as the top-level parallelism. This can
               be useful to pass a `-j` parameter to a system like `make`. Note
               that care should be taken when interpreting this environment
               variable. For historical purposes this is still provided but
               recent versions of Cargo, for example, do not need to run `make
               -j`, and instead can set the `MAKEFLAGS` env var to the content
               of `CARGO_MAKEFLAGS` to activate the use of Cargo's GNU Make
               compatible [jobserver] for sub-make invocations.
* `OPT_LEVEL`, `DEBUG` --- values of the corresponding variables for the
                         profile currently being built.
* `PROFILE` --- `release` for release builds, `debug` for other builds. This is
  determined based on if the [profile] inherits from the [`dev`] or
  [`release`] profile. Using this environment variable is not recommended.
  Using other environment variables like `OPT_LEVEL` provide a more correct
  view of the actual settings being used.
* `DEP_<name>_<key>` --- For more information about this set of environment
                       variables, see build script documentation about [`links`][links].
* `RUSTC`, `RUSTDOC` --- the compiler and documentation generator that Cargo has
                       resolved to use, passed to the build script so it might
                       use it as well.
* `RUSTC_WRAPPER` --- the `rustc` wrapper, if any, that Cargo is using.
                    See [`build.rustc-wrapper`].
* `RUSTC_WORKSPACE_WRAPPER` --- the `rustc` wrapper, if any, that Cargo is
			      using for workspace members. See
			      [`build.rustc-workspace-wrapper`].
* `RUSTC_LINKER` --- The path to the linker binary that Cargo has resolved to use
                   for the current target, if specified. The linker can be
                   changed by editing `.cargo/config.toml`; see the documentation
                   about [cargo configuration][cargo-config] for more
                   information.
* `CARGO_ENCODED_RUSTFLAGS` --- extra flags that Cargo invokes `rustc` with,
  separated by a `0x1f` character (ASCII Unit Separator). See
  [`build.rustflags`]. Note that since Rust 1.55, `RUSTFLAGS` is removed from
  the environment; scripts should use `CARGO_ENCODED_RUSTFLAGS` instead.
* `CARGO_PKG_<var>` --- The package information variables, with the same names and values as are [provided during crate building][variables set for crates].

[`tracing`]: https://docs.rs/tracing
[debug logging]: https://doc.crates.io/contrib/architecture/console.html#debug-logging
[unix-like platforms]: ../../reference/conditional-compilation.html#unix-and-windows
[windows-like platforms]: ../../reference/conditional-compilation.html#unix-and-windows
[target family]: ../../reference/conditional-compilation.html#target_family
[target operating system]: ../../reference/conditional-compilation.html#target_os
[target architecture]: ../../reference/conditional-compilation.html#target_arch
[target vendor]: ../../reference/conditional-compilation.html#target_vendor
[target environment]: ../../reference/conditional-compilation.html#target_env
[pointer width]: ../../reference/conditional-compilation.html#target_pointer_width
[target endianness]: ../../reference/conditional-compilation.html#target_endian
[target features]: ../../reference/conditional-compilation.html#target_feature
[links]: build-scripts.md#the-links-manifest-key
[configuration]: ../../reference/conditional-compilation.html
[jobserver]: https://www.gnu.org/software/make/manual/html_node/Job-Slots.html
[cargo-config]: config.md
[Target Triple]: ../appendix/glossary.md#target
[variables set for crates]: #environment-variables-cargo-sets-for-crates
[profile]: profiles.md
[`dev`]: profiles.md#dev
[`release`]: profiles.md#release

## Environment variables Cargo sets for 3rd party subcommands

Cargo exposes this environment variable to 3rd party subcommands
(ie. programs named `cargo-foobar` placed in `$PATH`):

* `CARGO` --- Path to the `cargo` binary performing the build.

For extended information about your environment you may run `cargo metadata`.
