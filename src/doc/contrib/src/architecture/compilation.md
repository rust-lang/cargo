# Compilation

The [`Unit`] is the primary data structure representing a single execution of
the compiler. It (mostly) contains all the information needed to determine
which flags to pass to the compiler.

The entry to the compilation process is located in the [`cargo_compile`]
module. The compilation can be conceptually broken into these steps:

1. Perform dependency resolution (see [the resolution chapter]).
2. Generate the root `Unit`s, the things the user requested to compile on the
   command-line. This is done in [`generate_targets`].
3. Starting from the root `Unit`s, generate the [`UnitGraph`] by walking the
   dependency graph from the resolver. The `UnitGraph` contains all of the
   `Unit` structs, and information about the dependency relationships between
   units. This is done in the [`unit_dependencies`] module.
4. Construct the [`BuildContext`] with all of the information collected so
   far. This is the end of the "front end" of compilation.
5. Create a [`Context`], a large, mutable data structure that coordinates the
   compilation process.
6. The [`Context`] will create a [`JobQueue`], a data structure that tracks
   which units need to be built.
7. [`drain_the_queue`] does the compilation process. This is the only point in
   Cargo that currently uses threads.
8. The result of the compilation is stored in the [`Compilation`] struct. This
   can be used for various things, such as running tests after the compilation
   has finished.

[`cargo_compile`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/ops/cargo_compile.rs
[`generate_targets`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/ops/cargo_compile.rs#L725-L739
[`UnitGraph`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/unit_graph.rs
[the resolution chapter]: packages.md
[`Unit`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/unit.rs
[`unit_dependencies`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/unit_dependencies.rs
[`BuildContext`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/build_context/mod.rs
[`Context`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/context/mod.rs
[`JobQueue`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/job_queue.rs
[`drain_the_queue`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/core/compiler/job_queue.rs#L623-L634
[`Compilation`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/compilation.rs
