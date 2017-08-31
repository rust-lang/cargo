% External tools

One of the goals of Cargo is simple integration with third-party tools, like
IDEs and other build systems. To make integration easier, Cargo has several
facilities:

* `cargo metadata` command, which outputs project structure and dependencies
  information in JSON,

* `--message-format` flag, which outputs information about a particular build,

* support for custom subcommands.


# Information about project structure

You can use `cargo metadata` command to get information about project structure
and dependencies. The output of the command looks like this:

```text
{
  // Integer version number of the format.
  "version": integer,

  // List of packages for this workspace, including dependencies.
  "packages": [
    {
      // Opaque package identifier.
      "id": PackageId,

      "name": string,

      "version": string,

      "source": SourceId,

      // A list of declared dependencies, see `resolve` field for actual dependencies.
      "dependencies": [ Dependency ],

      "targets: [ Target ],

      // Path to Cargo.toml
      "manifest_path": string,
    }
  ],

  "workspace_members": [ PackageId ],

  // Dependencies graph.
  "resolve": {
     "nodes": [
       {
         "id": PackageId,
         "dependencies": [ PackageId ]
       }
     ]
  }
}
```

The format is stable and versioned. When calling `cargo metadata`, you should
pass `--format-version` flag explicitly to avoid forward incompatibility
hazard.

If you are using Rust, there is [cargo_metadata] crate.

[cargo_metadata]: https://crates.io/crates/cargo_metadata


# Information about build

When passing `--message-format=json`, Cargo will output the following
information during the build:

* compiler errors and warnings,

* produced artifacts,

* results of the build scripts (for example, native dependencies).

The output goes to stdout in the JSON object per line format. The `reason` field
distinguishes different kinds of messages.

Information about dependencies in the Makefile-compatible format is stored in
the `.d` files alongside the artifacts.


# Custom subcommands.

Cargo is designed to be extensible with new subcommands without having to modify
Cargo itself. This is achieved by translating a cargo invocation of the form
cargo `(?<command>[^ ]+)` into an invocation of an external tool
`cargo-${command}` that then needs to be present in one of the user's `$PATH`
directories.

Custom subcommand may use `CARGO` environment variable to call back to
Cargo. Alternatively, it can link to `cargo` crate as a library, but this
approach has drawbacks:

* Cargo as a library is unstable, API changes without deprecation,

* versions of Cargo library and Cargo binary may be different.
