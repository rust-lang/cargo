# Profiling

## Internal profiler

Cargo has a basic, hierarchical profiler built-in. The environment variable
`CARGO_PROFILE` can be set to an integer which specifies how deep in the
profile stack to print results for.

```sh
# Output first three levels of profiling info
CARGO_PROFILE=3 cargo generate-lockfile
```

## Informal profiling

The overhead for starting a build should be kept as low as possible
(preferably, well under 0.5 seconds on most projects and systems). Currently,
the primary parts that affect this are:

* Running the resolver.
* Querying the index.
* Checking git dependencies.
* Scanning the local project.
* Building the unit dependency graph.

We currently don't have any automated systems or tools for measuring or
tracking the startup time. We informally measure these on changes that are
likely to affect the performance. Usually this is done by measuring the time
for `cargo build` to finish in a large project where the build is fresh (no
actual compilation is performed). [Hyperfine] is a command-line tool that can
be used to roughly measure the difference between different commands and
settings.

[Hyperfine]: https://github.com/sharkdp/hyperfine
