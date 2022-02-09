### Future incompat report

Cargo checks for  future-incompatible warnings in all dependencies. These are warnings for
changes that may become hard errors in the future, causing the dependency to
stop building in a future version of rustc. If any warnings are found, a small
notice is displayed indicating that the warnings were found, and provides
instructions on how to display a full report.

A full report can be displayed with the `cargo report future-incompatibilities
--id ID` command, or by running the build again with
the `--future-incompat-report` flag. The developer should then update their
dependencies to a version where the issue is fixed, or work with the
developers of the dependencies to help resolve the issue.

This feature can be configured through a `[future-incompat-report]`
section in `.cargo/config`. Currently, the supported options are:

```
[future-incompat-report]
frequency = FREQUENCY
```

The supported values for `FREQUENCY` are `always` and `never`, which control
whether or not a message is printed out at the end of `cargo build` / `cargo check`.
