# Cargo Benchmarking

This directory contains some benchmarks for cargo itself. This uses
[Criterion] for running benchmarks. It is recommended to read the Criterion
book to get familiar with how to use it. A basic usage would be:

```sh
cd benches/benchsuite
cargo bench
```

The tests involve downloading the index and benchmarking against some
real-world and artificial workspaces located in the [`workspaces`](workspaces)
directory.

**Beware** that the initial download can take a fairly long amount of time (10
minutes minimum on an extremely fast network) and require significant disk
space (around 4.5GB). The benchsuite will cache the index and downloaded
crates in the `target/tmp/bench` directory, so subsequent runs should be
faster. You can (and probably should) specify individual benchmarks to run to
narrow it down to a more reasonable set, for example:

```sh
cargo bench -- resolve_ws/rust
```

This will only download what's necessary for the rust-lang/rust workspace
(which is about 330MB) and run the benchmarks against it (which should take
about a minute). To get a list of all the benchmarks, run:

```sh
cargo bench -- --list
```

## Viewing reports

The benchmarks display some basic information on the command-line while they
run. A more complete HTML report can be found at
`target/criterion/report/index.html` which contains links to all the
benchmarks and summaries. Check out the Criterion book for more information on
the extensive reporting capabilities.

## Comparing implementations

Knowing the raw numbers can be useful, but what you're probably most
interested in is checking if your changes help or hurt performance. To do
that, you need to run the benchmarks multiple times.

First, run the benchmarks from the master branch of cargo without any changes.
To make it easier to compare, Criterion supports naming the baseline so that
you can iterate on your code and compare against it multiple times.

```sh
cargo bench -- --save-baseline master
```

Now you can switch to your branch with your changes. Re-run the benchmarks
compared against the baseline:

```sh
cargo bench -- --baseline master
```

You can repeat the last command as you make changes to re-compare against the
master baseline.

Without the baseline arguments, it will compare against the last run, which
can be helpful for comparing incremental changes.

## Capturing workspaces

The [`workspaces`](workspaces) directory contains several workspaces that
provide a variety of different workspaces intended to provide good exercises
for benchmarks. Some of these are shadow copies of real-world workspaces. This
is done with the tool in the [`capture`](capture) directory. The tool will
copy `Cargo.lock` and all of the `Cargo.toml` files of the workspace members.
It also adds an empty `lib.rs` so Cargo won't error, and sanitizes the
`Cargo.toml` to some degree, removing unwanted elements. Finally, it
compresses everything into a `tgz`.

To run it, do:

```sh
cd benches/capture
cargo run -- /path/to/workspace/foo
```

The resolver benchmarks also support the `CARGO_BENCH_WORKSPACES` environment
variable, which you can point to a Cargo workspace if you want to try
different workspaces. For example:

```sh
CARGO_BENCH_WORKSPACES=/path/to/some/workspace cargo bench
```

## TODO

This is just a start for establishing a benchmarking suite for Cargo. There's
a lot that can be added. Some ideas:

* Fix the benchmarks so that the resolver setup doesn't run every iteration.
* Benchmark [this section of
  code](https://github.com/rust-lang/cargo/blob/a821e2cb24d7b6013433f069ab3bad53d160e100/src/cargo/ops/cargo_compile.rs#L470-L549)
  which builds the unit graph. The performance there isn't great, and it would
  be good to keep an eye on it. Unfortunately that would mean doing a bit of
  work to make `generate_targets` publicly visible, and there is a bunch of
  setup code that may need to be duplicated.
* Benchmark the fingerprinting code.
* Benchmark running the `cargo` executable. Running something like `cargo
  build` or `cargo check` with everything "Fresh" would be a good end-to-end
  exercise to measure the overall overhead of Cargo.
* Benchmark pathological resolver scenarios. There might be some cases where
  the resolver can spend a significant amount of time. It would be good to
  identify if these exist, and create benchmarks for them. This may require
  creating an artificial index, similar to the `resolver-tests`. This should
  also consider scenarios where the resolver ultimately fails.
* Benchmark without `Cargo.lock`. I'm not sure if this is particularly
  valuable, since we are mostly concerned with incremental builds which will
  always have a lock file.
* Benchmark just
  [`resolve::resolve`](https://github.com/rust-lang/cargo/blob/a821e2cb24d7b6013433f069ab3bad53d160e100/src/cargo/core/resolver/mod.rs#L122)
  without anything else. This can help focus on just the resolver.

[Criterion]: https://bheisler.github.io/criterion.rs/book/
