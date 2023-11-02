# Debugging

## Logging

Cargo uses the [`tracing`] crate to display debug log messages.
The `CARGO_LOG` environment variable can be set to enable debug logging, with a value such as `trace`, `debug`, or `warn`.
It also supports filtering for specific modules with comma-separated [directives].
Feel free to use [shorthand macros] to help with diagnosing problems.
We're looking forward to making Cargo logging mechanism more structural!

```sh
# Outputs all logs with levels debug and higher
CARGO_LOG=debug cargo generate-lockfile

# Don't forget that you can filter by module as well
CARGO_LOG=cargo::core::resolver=trace cargo generate-lockfile

# This will print lots of info about the download process. `trace` prints even more.
CARGO_HTTP_DEBUG=true CARGO_LOG=network=debug cargo fetch

# This is an important command for diagnosing fingerprint issues.
CARGO_LOG=cargo::core::compiler::fingerprint=trace cargo build
```

[`tracing`]: https://docs.rs/tracing
[directive]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives
[shorthand macros]: https://docs.rs/tracing/latest/tracing/index.html#shorthand-macros
