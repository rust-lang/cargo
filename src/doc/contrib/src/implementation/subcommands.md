# New Subcommands

Cargo is a single binary composed of a set of [`clap`] subcommands. All
subcommands live in [`src/bin/cargo/commands`] directory.
[`src/bin/cargo/main.rs`] is the entry point.

Each subcommand, such as [`src/bin/cargo/commands/build.rs`], usually performs
the following:

1. Parse the CLI flags. See the [`command_prelude`] module for some helpers to make this easier.
2. Load the config files.
3. Discover and load the workspace.
4. Calls the actual implementation of the subcommand which resides in [`src/cargo/ops`].

If the subcommand is not found in the built-in list, then Cargo will
automatically search for a subcommand named `cargo-{NAME}` in the users `PATH`
to execute the subcommand.


[`clap`]: https://clap.rs/
[`src/bin/cargo/commands/build.rs`]: https://github.com/rust-lang/cargo/tree/master/src/bin/cargo/commands/build.rs
[`src/cargo/ops`]: https://github.com/rust-lang/cargo/tree/master/src/cargo/ops
[`src/bin/cargo/commands`]: https://github.com/rust-lang/cargo/tree/master/src/bin/cargo/commands
[`src/bin/cargo/main.rs`]: https://github.com/rust-lang/cargo/blob/master/src/bin/cargo/main.rs
[`command_prelude`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/util/command_prelude.rs
