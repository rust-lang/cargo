# Benchmarking and Profiling

## Internal profiler

Cargo has a basic, hierarchical profiler built-in. The environment variable
`CARGO_PROFILE` can be set to an integer which specifies how deep in the
profile stack to print results for.

```sh
# Output first three levels of profiling info
CARGO_PROFILE=3 cargo generate-lockfile
```

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
