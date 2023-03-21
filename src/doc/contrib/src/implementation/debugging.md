# Debugging

## Logging

Cargo uses the [`env_logger`] crate to display debug log messages. The
`CARGO_LOG` environment variable can be set to enable debug logging, with a
value such as `trace`, `debug`, or `warn`. It also supports filtering for
specific modules. Feel free to use the standard [`log`] macros to help with
diagnosing problems.

```sh
# Outputs all logs with levels debug and higher
CARGO_LOG=debug cargo generate-lockfile

# Don't forget that you can filter by module as well
CARGO_LOG=cargo::core::resolver=trace cargo generate-lockfile

# This will print lots of info about the download process. `trace` prints even more.
CARGO_HTTP_DEBUG=true CARGO_LOG=cargo::ops::registry=debug cargo fetch

# This is an important command for diagnosing fingerprint issues.
CARGO_LOG=cargo::core::compiler::fingerprint=trace cargo build
```

[`env_logger`]: https://docs.rs/env_logger
[`log`]: https://docs.rs/log
