{{#option "`--timings=`_fmts_"}}
Output information how long each compilation takes, and track concurrency
information over time. Accepts an optional comma-separated list of output
formats; `--timings` without an argument will default to `--timings=html`.
Specifying an output format (rather than the default) is unstable and requires
`-Zunstable-options`. Valid output formats:

- `html` (unstable, requires `-Zunstable-options`): Write a human-readable file `cargo-timing.html` to the
  `target/cargo-timings` directory with a report of the compilation. Also write
  a report to the same directory with a timestamp in the filename if you want
  to look at older runs. HTML output is suitable for human consumption only,
  and does not provide machine-readable timing data.
- `json` (unstable, requires `-Zunstable-options`): Emit machine-readable JSON
  information about timing information.
{{/option}}

