# cargo-metadata(1)

## NAME

cargo-metadata - Machine-readable metadata about the current package

## SYNOPSIS

`cargo metadata` [_options_]

## DESCRIPTION

Output JSON to stdout containing information about the workspace members and
resolved dependencies of the current package.

It is recommended to include the `--format-version` flag to future-proof
your code to ensure the output is in the format you are expecting.

See the [cargo_metadata crate](https://crates.io/crates/cargo_metadata)
for a Rust API for reading the metadata.

## OUTPUT FORMAT

The output has the following format:

```javascript
{
    /* Array of all packages in the workspace.
       It also includes all feature-enabled dependencies unless --no-deps is used.
    */
    "packages": [
        {
            /* The name of the package. */
            "name": "my-package",
            /* The version of the package. */
            "version": "0.1.0",
            /* The Package ID, a unique identifier for referring to the package. */
            "id": "my-package 0.1.0 (path+file:///path/to/my-package)",
            /* The license value from the manifest, or null. */
            "license": "MIT/Apache-2.0",
            /* The license-file value from the manifest, or null. */
            "license_file": "LICENSE",
            /* The description value from the manifest, or null. */
            "description": "Package description.",
            /* The source ID of the package. This represents where
               a package is retrieved from.
               This is null for path dependencies and workspace members.
               For other dependencies, it is a string with the format:
               - "registry+URL" for registry-based dependencies.
                 Example: "registry+https://github.com/rust-lang/crates.io-index"
               - "git+URL" for git-based dependencies.
                 Example: "git+https://github.com/rust-lang/cargo?rev=5e85ba14aaa20f8133863373404cb0af69eeef2c#5e85ba14aaa20f8133863373404cb0af69eeef2c"
            */
            "source": null,
            /* Array of dependencies declared in the package's manifest. */
            "dependencies": [
                {
                    /* The name of the dependency. */
                    "name": "bitflags",
                    /* The source ID of the dependency. May be null, see
                       description for the package source.
                    */
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    /* The version requirement for the dependency.
                       Dependencies without a version requirement have a value of "*".
                    */
                    "req": "^1.0",
                    /* The dependency kind.
                       "dev", "build", or null for a normal dependency.
                    */
                    "kind": null,
                    /* If the dependency is renamed, this is the new name for
                       the dependency as a string.  null if it is not renamed.
                    */
                    "rename": null,
                    /* Boolean of whether or not this is an optional dependency. */
                    "optional": false,
                    /* Boolean of whether or not default features are enabled. */
                    "uses_default_features": true,
                    /* Array of features enabled. */
                    "features": [],
                    /* The target platform for the dependency.
                       null if not a target dependency.
                    */
                    "target": "cfg(windows)",
                    /* The file system path for a local path dependency.
                       not present if not a path dependency.
                    */
                    "path": "/path/to/dep",
                    /* A string of the URL of the registry this dependency is from.
                       If not specified or null, the dependency is from the default
                       registry (crates.io).
                    */
                    "registry": null
                }
            ],
            /* Array of Cargo targets. */
            "targets": [
                {
                    /* Array of target kinds.
                       - lib targets list the `crate-type` values from the
                         manifest such as "lib", "rlib", "dylib",
                         "proc-macro", etc. (default ["lib"])
                       - binary is ["bin"]
                       - example is ["example"]
                       - integration test is ["test"]
                       - benchmark is ["bench"]
                       - build script is ["custom-build"]
                    */
                    "kind": [
                        "bin"
                    ],
                    /* Array of crate types.
                       - lib and example libraries list the `crate-type` values
                         from the manifest such as "lib", "rlib", "dylib",
                         "proc-macro", etc. (default ["lib"])
                       - all other target kinds are ["bin"]
                    */
                    "crate_types": [
                        "bin"
                    ],
                    /* The name of the target. */
                    "name": "my-package",
                    /* Absolute path to the root source file of the target. */
                    "src_path": "/path/to/my-package/src/main.rs",
                    /* The Rust edition of the target.
                       Defaults to the package edition.
                    */
                    "edition": "2018",
                    /* Array of required features.
                       This property is not included if no required features are set.
                    */
                    "required-features": ["feat1"],
                    /* Whether the target should be documented by `cargo doc`. */
                    "doc": true,
                    /* Whether or not this target has doc tests enabled, and
                       the target is compatible with doc testing.
                    */
                    "doctest": false,
                    /* Whether or not this target should be built and run with `--test`
                    */
                    "test": true
                }
            ],
            /* Set of features defined for the package.
               Each feature maps to an array of features or dependencies it
               enables.
            */
            "features": {
                "default": [
                    "feat1"
                ],
                "feat1": [],
                "feat2": []
            },
            /* Absolute path to this package's manifest. */
            "manifest_path": "/path/to/my-package/Cargo.toml",
            /* Package metadata.
               This is null if no metadata is specified.
            */
            "metadata": {
                "docs": {
                    "rs": {
                        "all-features": true
                    }
                }
            },
            /* List of registries to which this package may be published.
               Publishing is unrestricted if null, and forbidden if an empty array. */
            "publish": [
                "crates-io"
            ],
            /* Array of authors from the manifest.
               Empty array if no authors specified.
            */
            "authors": [
                "Jane Doe <user@example.com>"
            ],
            /* Array of categories from the manifest. */
            "categories": [
                "command-line-utilities"
            ],
            /* Optional string that is the default binary picked by cargo run. */
            "default_run": null,
            /* Optional string that is the minimum supported rust version */
            "rust_version": "1.56",
            /* Array of keywords from the manifest. */
            "keywords": [
                "cli"
            ],
            /* The readme value from the manifest or null if not specified. */
            "readme": "README.md",
            /* The repository value from the manifest or null if not specified. */
            "repository": "https://github.com/rust-lang/cargo",
            /* The homepage value from the manifest or null if not specified. */
            "homepage": "https://rust-lang.org",
            /* The documentation value from the manifest or null if not specified. */
            "documentation": "https://doc.rust-lang.org/stable/std",
            /* The default edition of the package.
               Note that individual targets may have different editions.
            */
            "edition": "2018",
            /* Optional string that is the name of a native library the package
               is linking to.
            */
            "links": null,
        }
    ],
    /* Array of members of the workspace.
       Each entry is the Package ID for the package.
    */
    "workspace_members": [
        "my-package 0.1.0 (path+file:///path/to/my-package)",
    ],
    // The resolved dependency graph for the entire workspace. The enabled
    // features are based on the enabled features for the "current" package.
    // Inactivated optional dependencies are not listed.
    //
    // This is null if --no-deps is specified.
    //
    // By default, this includes all dependencies for all target platforms.
    // The `--filter-platform` flag may be used to narrow to a specific
    // target triple.
    "resolve": {
        /* Array of nodes within the dependency graph.
           Each node is a package.
        */
        "nodes": [
            {
                /* The Package ID of this node. */
                "id": "my-package 0.1.0 (path+file:///path/to/my-package)",
                /* The dependencies of this package, an array of Package IDs. */
                "dependencies": [
                    "bitflags 1.0.4 (registry+https://github.com/rust-lang/crates.io-index)"
                ],
                /* The dependencies of this package. This is an alternative to
                   "dependencies" which contains additional information. In
                   particular, this handles renamed dependencies.
                */
                "deps": [
                    {
                        /* The name of the dependency's library target.
                           If this is a renamed dependency, this is the new
                           name.
                        */
                        "name": "bitflags",
                        /* The Package ID of the dependency. */
                        "pkg": "bitflags 1.0.4 (registry+https://github.com/rust-lang/crates.io-index)",
                        /* Array of dependency kinds. Added in Cargo 1.40. */
                        "dep_kinds": [
                            {
                                /* The dependency kind.
                                   "dev", "build", or null for a normal dependency.
                                */
                                "kind": null,
                                /* The target platform for the dependency.
                                   null if not a target dependency.
                                */
                                "target": "cfg(windows)"
                            }
                        ]
                    }
                ],
                /* Array of features enabled on this package. */
                "features": [
                    "default"
                ]
            }
        ],
        /* The root package of the workspace.
           This is null if this is a virtual workspace. Otherwise it is
           the Package ID of the root package.
        */
        "root": "my-package 0.1.0 (path+file:///path/to/my-package)"
    },
    /* The absolute path to the build directory where Cargo places its output. */
    "target_directory": "/path/to/my-package/target",
    /* The version of the schema for this metadata structure.
       This will be changed if incompatible changes are ever made.
    */
    "version": 1,
    /* The absolute path to the root of the workspace. */
    "workspace_root": "/path/to/my-package"
    /* Workspace metadata.
       This is null if no metadata is specified. */
    "metadata": {
        "docs": {
            "rs": {
                "all-features": true
            }
        }
    }
}
````

## OPTIONS

### Output Options

<dl>

<dt class="option-term" id="option-cargo-metadata---no-deps"><a class="option-anchor" href="#option-cargo-metadata---no-deps"></a><code>--no-deps</code></dt>
<dd class="option-desc">Output information only about the workspace members and don't fetch
dependencies.</dd>


<dt class="option-term" id="option-cargo-metadata---format-version"><a class="option-anchor" href="#option-cargo-metadata---format-version"></a><code>--format-version</code> <em>version</em></dt>
<dd class="option-desc">Specify the version of the output format to use. Currently <code>1</code> is the only
possible value.</dd>


<dt class="option-term" id="option-cargo-metadata---filter-platform"><a class="option-anchor" href="#option-cargo-metadata---filter-platform"></a><code>--filter-platform</code> <em>triple</em></dt>
<dd class="option-desc">This filters the <code>resolve</code> output to only include dependencies for the
given target triple. Without this flag, the resolve includes all targets.</p>
<p>Note that the dependencies listed in the &quot;packages&quot; array still includes all
dependencies. Each package definition is intended to be an unaltered
reproduction of the information within <code>Cargo.toml</code>.</dd>


</dl>

### Feature Selection

The feature flags allow you to control which features are enabled. When no
feature options are given, the `default` feature is activated for every
selected package.

See [the features documentation](../reference/features.html#command-line-feature-options)
for more details.

<dl>

<dt class="option-term" id="option-cargo-metadata---features"><a class="option-anchor" href="#option-cargo-metadata---features"></a><code>--features</code> <em>features</em></dt>
<dd class="option-desc">Space or comma separated list of features to activate. Features of workspace
members may be enabled with <code>package-name/feature-name</code> syntax. This flag may
be specified multiple times, which enables all specified features.</dd>


<dt class="option-term" id="option-cargo-metadata---all-features"><a class="option-anchor" href="#option-cargo-metadata---all-features"></a><code>--all-features</code></dt>
<dd class="option-desc">Activate all available features of all selected packages.</dd>


<dt class="option-term" id="option-cargo-metadata---no-default-features"><a class="option-anchor" href="#option-cargo-metadata---no-default-features"></a><code>--no-default-features</code></dt>
<dd class="option-desc">Do not activate the <code>default</code> feature of the selected packages.</dd>


</dl>


### Display Options

<dl>
<dt class="option-term" id="option-cargo-metadata--v"><a class="option-anchor" href="#option-cargo-metadata--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-metadata---verbose"><a class="option-anchor" href="#option-cargo-metadata---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for &quot;very verbose&quot; output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-metadata--q"><a class="option-anchor" href="#option-cargo-metadata--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-metadata---quiet"><a class="option-anchor" href="#option-cargo-metadata---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-metadata---color"><a class="option-anchor" href="#option-cargo-metadata---color"></a><code>--color</code> <em>when</em></dt>
<dd class="option-desc">Control when colored output is used. Valid values:</p>
<ul>
<li><code>auto</code> (default): Automatically detect if color support is available on the
terminal.</li>
<li><code>always</code>: Always display colors.</li>
<li><code>never</code>: Never display colors.</li>
</ul>
<p>May also be specified with the <code>term.color</code>
<a href="../reference/config.html">config value</a>.</dd>


</dl>

### Manifest Options

<dl>
<dt class="option-term" id="option-cargo-metadata---manifest-path"><a class="option-anchor" href="#option-cargo-metadata---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>



<dt class="option-term" id="option-cargo-metadata---frozen"><a class="option-anchor" href="#option-cargo-metadata---frozen"></a><code>--frozen</code></dt>
<dt class="option-term" id="option-cargo-metadata---locked"><a class="option-anchor" href="#option-cargo-metadata---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Either of these flags requires that the <code>Cargo.lock</code> file is
up-to-date. If the lock file is missing, or it needs to be updated, Cargo will
exit with an error. The <code>--frozen</code> flag also prevents Cargo from
attempting to access the network to determine if it is out-of-date.</p>
<p>These may be used in environments where you want to assert that the
<code>Cargo.lock</code> file is up-to-date (such as a CI build) or want to avoid network
access.</dd>


<dt class="option-term" id="option-cargo-metadata---offline"><a class="option-anchor" href="#option-cargo-metadata---offline"></a><code>--offline</code></dt>
<dd class="option-desc">Prevents Cargo from accessing the network for any reason. Without this
flag, Cargo will stop with an error if it needs to access the network and
the network is not available. With this flag, Cargo will attempt to
proceed without the network if possible.</p>
<p>Beware that this may result in different dependency resolution than online
mode. Cargo will restrict itself to crates that are downloaded locally, even
if there might be a newer version as indicated in the local copy of the index.
See the <a href="cargo-fetch.html">cargo-fetch(1)</a> command to download dependencies before going
offline.</p>
<p>May also be specified with the <code>net.offline</code> <a href="../reference/config.html">config value</a>.</dd>


</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-metadata-+toolchain"><a class="option-anchor" href="#option-cargo-metadata-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-metadata--h"><a class="option-anchor" href="#option-cargo-metadata--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-metadata---help"><a class="option-anchor" href="#option-cargo-metadata---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-metadata--Z"><a class="option-anchor" href="#option-cargo-metadata--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>


## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.


## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.


## EXAMPLES

1. Output JSON about the current package:

       cargo metadata --format-version=1

## SEE ALSO
[cargo(1)](cargo.html)
