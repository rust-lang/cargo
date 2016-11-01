% Machine readable output.

Cargo can output information about project and build in JSON format.

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


# Compiler errors

If you supply `--message-format json` to commands like `cargo build`, Cargo
reports compilation errors and warnings in JSON format. Messages go to the
standard output. Each message occupies exactly one line and does not contain
internal `\n` symbols, so it is possible to process messages one by one
without waiting for the whole build to finish.

The message format looks like this:

```text
{
  // Type of the message.
  "reason": "compiler-message",

  // Unique opaque identifier of compiled package.
  "package_id": PackageId,

  // Unique specification of a particular target within the package.
  "target": Target,

  // The error message from the compiler in JSON format.
  "message": {...}
}
```

Package and target specification are the same that `cargo metadata` uses.
