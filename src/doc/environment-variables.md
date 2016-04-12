% Environment Variables

Cargo sets and reads a number of environment variables which your code can detect
or override. Here is a list of the variables Cargo sets, organized by when it interacts
with them:

# Environment variables Cargo reads

You can override these environment variables to change Cargo's behavior on your
system:

* `CARGO_HOME` - Cargo maintains a local cache of the registry index and of git
  checkouts of crates.  By default these are stored under `$HOME/.cargo`, but
  this variable overrides the location of this directory. Once a crate is cached
  it is not removed by the clean command.
* `CARGO_TARGET_DIR` - Location of where to place all generated artifacts,
  relative to the current working directory.
* `RUSTC` - Instead of running `rustc`, Cargo will execute this specified
  compiler instead.
* `RUSTDOC` - Instead of running `rustdoc`, Cargo will execute this specified
  `rustdoc` instance instead.
* `RUSTFLAGS` - A space-separated list of custom flags to pass to all compiler
  invocations that Cargo performs. In contrast with `cargo rustc`, this is
  useful for passing a flag to *all* compiler instances.

Note that Cargo will also read environment variables for `.cargo/config`
configuration values, as described in [that documentation][config-env]

[config-env]: config.html#environment-variables

# Environment variables Cargo sets for crates

Cargo exposes these environment variables to your crate when it is compiled. To get the
value of any of these variables in a Rust program, do this:

```
let version = env!("CARGO_PKG_VERSION");
```

`version` will now contain the value of `CARGO_PKG_VERSION`.

* `CARGO_MANIFEST_DIR` - The directory containing the manifest of your package.
* `CARGO_PKG_VERSION` - The full version of your package.
* `CARGO_PKG_VERSION_MAJOR` - The major version of your package.
* `CARGO_PKG_VERSION_MINOR` - The minor version of your package.
* `CARGO_PKG_VERSION_PATCH` - The patch version of your package.
* `CARGO_PKG_VERSION_PRE` - The pre-release version of your package.
* `CARGO_PKG_AUTHORS` - Colon seperated list of authors from the manifest of your package.
* `CARGO_PKG_NAME` - The name of your package.
* `CARGO_PKG_DESCRIPTION` - The description of your package.
* `CARGO_PKG_HOMEPAGE` - The home page of your package.

# Environment variables Cargo sets for build scripts

Cargo sets several environment variables when build scripts are run. Because these variables
are not yet set when the build script is compiled, the above example using `env!` won't work
and instead you'll need to retrieve the values when the build script is run:

```
use std::env;
let out_dir = env::var("OUT_DIR").unwrap();
```

`out_dir` will now contain the value of `OUT_DIR`.

* `CARGO_MANIFEST_DIR` - The directory containing the manifest for the package
                         being built (the package containing the build
                         script). Also note that this is the value of the
                         current working directory of the build script when it
                         starts.
* `CARGO_FEATURE_<name>` - For each activated feature of the package being
                           built, this environment variable will be present
                           where `<name>` is the name of the feature uppercased
                           and having `-` translated to `_`.
* `OUT_DIR` - the folder in which all output should be placed. This folder is
              inside the build directory for the package being built, and it is
              unique for the package in question.
* `TARGET` - the target triple that is being compiled for. Native code should be
             compiled for this triple. Some more information about target
             triples can be found in [clang’s own documentation][clang].
* `HOST` - the host triple of the rust compiler.
* `NUM_JOBS` - the parallelism specified as the top-level parallelism. This can
               be useful to pass a `-j` parameter to a system like `make`.
* `OPT_LEVEL`, `DEBUG` - values of the corresponding variables for the
                         profile currently being built.
* `PROFILE` - name of the profile currently being built (see
              [profiles][profile]).
* `DEP_<name>_<key>` - For more information about this set of environment
                       variables, see build script documentation about [`links`][links].

[links]: build-script.html#the-links-manifest-key
[profile]: manifest.html#the-profile-sections
[clang]:http://clang.llvm.org/docs/CrossCompilation.html#target-triple
