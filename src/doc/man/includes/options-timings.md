{{#option "`--timings`"}}
Output information how long each compilation takes, and track concurrency
information over time.

A file `cargo-timing.html` will be written to the `target/cargo-timings`
directory at the end of the build. An additional report with a timestamp
in its filename is also written if you want to look at a previous run.
These reports are suitable for human consumption only, and do not provide
machine-readable timing data.
{{/option}}

