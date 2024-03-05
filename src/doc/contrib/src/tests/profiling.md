# Benchmarking and Profiling

## Internal profiler

Cargo leverages [tracing](https://crates.io/crates/tracing)
as a basic, hierarchical built-in profiler.

Environment variables:
- `CARGO_LOG_PROFILE=<true|1>`: log tracing events to a file in the current working directory
- `CARGO_LOG_PROFILE_CAPTURE_ARGS=<true|1>`: include arguments in the events

At process exit, your trace will be in a file like `trace-1668480819035032.json`.
Open that file with [ui.perfetto.dev](https://ui.perfetto.dev) (or chrome://tracing) to browse your trace.

Example:
```console
$ # Output first three levels of profiling info
$ CARGO_LOG_PROFILE=true cargo generate-lockfile
```

**Note:** This is intended for the development of cargo and there are no compatibility guarantees on this functionality.

## Benchmarking

### Benchsuite

Head over to the [`benches`
directory](https://github.com/rust-lang/cargo/tree/master/benches) for more
information about the benchmarking suite.

### Informal benchmarking

The overhead for starting a build should be kept as low as possible
(preferably, well under 0.5 seconds on most projects and systems). Currently,
the primary parts that affect this are:

* Running the resolver.
* Querying the index.
* Checking git dependencies.
* Scanning the local project.
* Building the unit dependency graph.

One way to test this is to use [hyperfine]. This is a tool that can be used to
measure the difference between different commands and settings. Usually this
is done by measuring the time it takes for `cargo build` to finish in a large
project where the build is fresh (no actual compilation is performed). Just
run `cargo build` once before using hyperfine.

[hyperfine]: https://github.com/sharkdp/hyperfine
