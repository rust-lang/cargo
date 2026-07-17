# Future incompat report

Cargo checks for future-incompatible warnings in all dependencies. These are warnings for
changes that may become hard errors in the future, causing the dependency to
stop building in a future version of rustc. If any warnings are found, a small
notice is displayed indicating that the warnings were found, and provides
instructions on how to display a full report.

For example, you may see something like this at the end of a build:

```text
warning: the following packages contain code that will be rejected by a future
         version of Rust: rental v0.5.5
note: to see what the problems were, use the option `--future-incompat-report`,
      or run `cargo report future-incompatibilities --id 1`
```

A full report can be displayed with the `cargo report future-incompatibilities
--id ID` command, or by running the build again with
the `--future-incompat-report` flag. The developer should then update their
dependencies to a version where the issue is fixed, or work with the
developers of the dependencies to help resolve the issue.

## Configuration

This feature can be configured through a [`[future-incompat-report]`][config]
section in `.cargo/config.toml`. Currently, the supported options are:

```toml
[future-incompat-report]
frequency = "always"
```

The supported values for the frequency are `"always"` and `"never"`, which control
whether or not a message is printed out at the end of `cargo build` / `cargo check`.

[config]: config.md#future-incompat-report
