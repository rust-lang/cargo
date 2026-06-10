# Console Output

All of Cargo's output should go through the [`Shell`] struct. You can normally
obtain the `Shell` instance from the [`GlobalContext`] struct. Do **not** use
the std `println!` macros.

Most of Cargo's output goes to stderr. When running in JSON mode, the output
goes to stdout.

It is important to properly handle errors when writing to the console.
Informational commands, like `cargo list`, should ignore any errors writing
the output. There are some [`drop_print`] macros that are intended to make
this easier.

Messages written during compilation should handle errors, and abort the build
if they are unable to be displayed. This is generally automatically handled in
the [`JobQueue`] as it processes each message.

**Note:** keep the normal output brief.
Cargo is already fairly noisy,
so try to keep the output as brief and clean as possible.

[`Shell`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/shell.rs
[`GlobalContext`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/util/context/mod.rs
[`drop_print`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/util/config/mod.rs#L1820-L1848
[`JobQueue`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/job_queue/mod.rs

## Diagnostics

See [`diagnostics`](https://doc.rust-lang.org/nightly/nightly-rustc/cargo/diagnostics/index.html) for how to write diagnostics and lints (including errors)

### Errors

Cargo uses [`anyhow`] for managing errors. This makes it convenient to "chain"
errors together, so that Cargo can report how an error originated, and what it
was trying to do at the time.

Error helpers are implemented in the [`errors`] module. Use the
`InternalError` error type for errors that are not expected to happen. This
will print a message to the user to file a bug report.

The binary side of Cargo uses the `CliError` struct to wrap the process exit
code. Usually Cargo exits with 101 for an error, but some commands like `cargo
test` will exit with different codes.

**Note:** When using any low-level routines,
such as `std::fs`, *always* add error context about what it is doing.
For example, reading from a file should include context about which file is being read if there is an error.

[`anyhow`]: https://docs.rs/anyhow
[Errors and lints]: https://rustc-dev-guide.rust-lang.org/diagnostics.html
