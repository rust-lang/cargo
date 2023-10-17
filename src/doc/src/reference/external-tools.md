# External tools

One of the goals of Cargo is simple integration with third-party tools, like
IDEs and other build systems. To make integration easier, Cargo has several
facilities:

* a [`cargo metadata`] command, which outputs package structure and dependencies
  information in JSON,

* a `--message-format` flag, which outputs information about a particular build,
  and

* support for custom subcommands.


## Information about package structure

You can use [`cargo metadata`] command to get information about package
structure and dependencies. See the [`cargo metadata`] documentation
for details on the format of the output.

The format is stable and versioned. When calling `cargo metadata`, you should
pass `--format-version` flag explicitly to avoid forward incompatibility
hazard.

If you are using Rust, the [cargo_metadata] crate can be used to parse the
output.

[cargo_metadata]: https://crates.io/crates/cargo_metadata
[`cargo metadata`]: ../commands/cargo-metadata.md

## JSON messages

When passing `--message-format=json`, Cargo will output the following
information during the build:

* compiler errors and warnings,

* produced artifacts,

* results of the build scripts (for example, native dependencies).

The output goes to stdout in the JSON object per line format. The `reason` field
distinguishes different kinds of messages.

The `--message-format` option can also take additional formatting values which
alter the way the JSON messages are computed and rendered. See the description
of the `--message-format` option in the [build command documentation] for more
details.

If you are using Rust, the [cargo_metadata] crate can be used to parse these
messages.

[build command documentation]: ../commands/cargo-build.md
[cargo_metadata]: https://crates.io/crates/cargo_metadata

### Compiler messages

The "compiler-message" message includes output from the compiler, such as
warnings and errors. See the [rustc JSON chapter](../../rustc/json.md) for
details on `rustc`'s message format, which is embedded in the following
structure:

```javascript
{
    /* The "reason" indicates the kind of message. */
    "reason": "compiler-message",
    /* The Package ID, a unique identifier for referring to the package. */
    "package_id": "my-package 0.1.0 (path+file:///path/to/my-package)",
    /* Absolute path to the package manifest. */
    "manifest_path": "/path/to/my-package/Cargo.toml",
    /* The Cargo target (lib, bin, example, etc.) that generated the message. */
    "target": {
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
            "lib"
        ],
        /* Array of crate types.
           - lib and example libraries list the `crate-type` values
             from the manifest such as "lib", "rlib", "dylib",
             "proc-macro", etc. (default ["lib"])
           - all other target kinds are ["bin"]
        */
        "crate_types": [
            "lib"
        ],
        /* The name of the target. */
        "name": "my-package",
        /* Absolute path to the root source file of the target. */
        "src_path": "/path/to/my-package/src/lib.rs",
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
        "doctest": true
        /* Whether or not this target should be built and run with `--test`
        */
        "test": true
    },
    /* The message emitted by the compiler.

    See https://doc.rust-lang.org/rustc/json.html for details.
    */
    "message": {
        /* ... */
    }
}
```

### Artifact messages

For every compilation step, a "compiler-artifact" message is emitted with the
following structure:

```javascript
{
    /* The "reason" indicates the kind of message. */
    "reason": "compiler-artifact",
    /* The Package ID, a unique identifier for referring to the package. */
    "package_id": "my-package 0.1.0 (path+file:///path/to/my-package)",
    /* Absolute path to the package manifest. */
    "manifest_path": "/path/to/my-package/Cargo.toml",
    /* The Cargo target (lib, bin, example, etc.) that generated the artifacts.
       See the definition above for `compiler-message` for details.
    */
    "target": {
        "kind": [
            "lib"
        ],
        "crate_types": [
            "lib"
        ],
        "name": "my-package",
        "src_path": "/path/to/my-package/src/lib.rs",
        "edition": "2018",
        "doc": true,
        "doctest": true,
        "test": true
    },
    /* The profile indicates which compiler settings were used. */
    "profile": {
        /* The optimization level. */
        "opt_level": "0",
        /* The debug level, an integer of 0, 1, or 2, or a string
           "line-directives-only" or "line-tables-only". If `null`, it implies
           rustc's default of 0.
        */
        "debuginfo": 2,
        /* Whether or not debug assertions are enabled. */
        "debug_assertions": true,
        /* Whether or not overflow checks are enabled. */
        "overflow_checks": true,
        /* Whether or not the `--test` flag is used. */
        "test": false
    },
    /* Array of features enabled. */
    "features": ["feat1", "feat2"],
    /* Array of files generated by this step. */
    "filenames": [
        "/path/to/my-package/target/debug/libmy_package.rlib",
        "/path/to/my-package/target/debug/deps/libmy_package-be9f3faac0a26ef0.rmeta"
    ],
    /* A string of the path to the executable that was created, or null if
       this step did not generate an executable.
    */
    "executable": null,
    /* Whether or not this step was actually executed.
       When `true`, this means that the pre-existing artifacts were
       up-to-date, and `rustc` was not executed. When `false`, this means that
       `rustc` was run to generate the artifacts.
    */
    "fresh": true
}

```

### Build script output

The "build-script-executed" message includes the parsed output of a build
script. Note that this is emitted even if the build script is not run; it will
display the previously cached value. More details about build script output
may be found in [the chapter on build scripts](build-scripts.md).

```javascript
{
    /* The "reason" indicates the kind of message. */
    "reason": "build-script-executed",
    /* The Package ID, a unique identifier for referring to the package. */
    "package_id": "my-package 0.1.0 (path+file:///path/to/my-package)",
    /* Array of libraries to link, as indicated by the `cargo:rustc-link-lib`
       instruction. Note that this may include a "KIND=" prefix in the string
       where KIND is the library kind.
    */
    "linked_libs": ["foo", "static=bar"],
    /* Array of paths to include in the library search path, as indicated by
       the `cargo:rustc-link-search` instruction. Note that this may include a
       "KIND=" prefix in the string where KIND is the library kind.
    */
    "linked_paths": ["/some/path", "native=/another/path"],
    /* Array of cfg values to enable, as indicated by the `cargo:rustc-cfg`
       instruction.
    */
    "cfgs": ["cfg1", "cfg2=\"string\""],
    /* Array of [KEY, VALUE] arrays of environment variables to set, as
       indicated by the `cargo:rustc-env` instruction.
    */
    "env": [
        ["SOME_KEY", "some value"],
        ["ANOTHER_KEY", "another value"]
    ],
    /* An absolute path which is used as a value of `OUT_DIR` environmental
       variable when compiling current package.
    */
    "out_dir": "/some/path/in/target/dir"
}
```

### Build finished

The "build-finished" message is emitted at the end of the build.

```javascript
{
    /* The "reason" indicates the kind of message. */
    "reason": "build-finished",
    /* Whether or not the build finished successfully. */
    "success": true,
}
````

This message can be helpful for tools to know when to stop reading JSON
messages. Commands such as `cargo test` or `cargo run` can produce additional
output after the build has finished. This message lets a tool know that Cargo
will not produce additional JSON messages, but there may be additional output
that may be generated afterwards (such as the output generated by the program
executed by `cargo run`).

> Note: There is experimental nightly-only support for JSON output for tests,
> so additional test-specific JSON messages may begin arriving after the
> "build-finished" message if that is enabled.

## Custom subcommands

Cargo is designed to be extensible with new subcommands without having to modify
Cargo itself. This is achieved by translating a cargo invocation of the form
cargo `(?<command>[^ ]+)` into an invocation of an external tool
`cargo-${command}`. The external tool must be present in one of the user's
`$PATH` directories.

When Cargo invokes a custom subcommand, the first argument to the subcommand
will be the filename of the custom subcommand, as usual. The second argument
will be the subcommand name itself. For example, the second argument would be
`${command}` when invoking `cargo-${command}`. Any additional arguments on the
command line will be forwarded unchanged.

Cargo can also display the help output of a custom subcommand with `cargo help
${command}`. Cargo assumes that the subcommand will print a help message if its
third argument is `--help`. So, `cargo help ${command}` would invoke
`cargo-${command} ${command} --help`.

Custom subcommands may use the `CARGO` environment variable to call back to
Cargo. Alternatively, it can link to `cargo` crate as a library, but this
approach has drawbacks:

* Cargo as a library is unstable: the  API may change without deprecation
* versions of the linked Cargo library may be different from the Cargo binary

Instead, it is encouraged to use the CLI interface to drive Cargo. The [`cargo
metadata`] command can be used to obtain information about the current project
(the [`cargo_metadata`] crate provides a Rust interface to this command).

[`cargo metadata`]: ../commands/cargo-metadata.md
[`cargo_metadata`]: https://crates.io/crates/cargo_metadata
