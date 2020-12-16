Passing target selection flags will {{lower actionverb}} only the specified
targets. 

Note that `--bin`, `--example`, `--test` and `--bench` flags also 
support common Unix glob patterns like `*`, `?` and `[]`. However, to avoid your 
shell accidentally expanding glob patterns before Cargo handles them, you must 
use single quotes or double quotes around each glob pattern.

{{#options}}

{{> options-targets-lib-bin }}

{{#option "`--example` _name_..." }}
{{actionverb}} the specified example. This flag may be specified multiple times
and supports common Unix glob patterns.
{{/option}}

{{#option "`--examples`" }}
{{actionverb}} all example targets.
{{/option}}

{{#option "`--test` _name_..." }}
{{actionverb}} the specified integration test. This flag may be specified
multiple times and supports common Unix glob patterns.
{{/option}}

{{#option "`--tests`" }}
{{actionverb}} all targets in test mode that have the `test = true` manifest
flag set. By default this includes the library and binaries built as
unittests, and integration tests. Be aware that this will also build any
required dependencies, so the lib target may be built twice (once as a
unittest, and once as a dependency for binaries, integration tests, etc.).
Targets may be enabled or disabled by setting the `test` flag in the
manifest settings for the target.
{{/option}}

{{#option "`--bench` _name_..." }}
{{actionverb}} the specified benchmark. This flag may be specified multiple
times and supports common Unix glob patterns.
{{/option}}

{{#option "`--benches`" }}
{{actionverb}} all targets in benchmark mode that have the `bench = true`
manifest flag set. By default this includes the library and binaries built
as benchmarks, and bench targets. Be aware that this will also build any
required dependencies, so the lib target may be built twice (once as a
benchmark, and once as a dependency for binaries, benchmarks, etc.).
Targets may be enabled or disabled by setting the `bench` flag in the
manifest settings for the target.
{{/option}}

{{#option "`--all-targets`" }}
{{actionverb}} all targets. This is equivalent to specifying `--lib --bins
--tests --benches --examples`.
{{/option}}

{{/options}}
