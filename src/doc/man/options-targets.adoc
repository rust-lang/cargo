Passing target selection flags will convert:lowercase[{actionverb}] only the
specified targets.

include::options-targets-lib-bin.adoc[]

*--example* _NAME_...::
    {actionverb} the specified example. This flag may be specified multiple times.

*--examples*::
    {actionverb} all example targets.

*--test* _NAME_...::
    {actionverb} the specified integration test. This flag may be specified multiple
    times.

*--tests*::
    {actionverb} all targets in test mode that have the `test = true` manifest
    flag set. By default this includes the library and binaries built as
    unittests, and integration tests. Be aware that this will also build any
    required dependencies, so the lib target may be built twice (once as a
    unittest, and once as a dependency for binaries, integration tests, etc.).
    Targets may be enabled or disabled by setting the `test` flag in the
    manifest settings for the target.

*--bench* _NAME_...::
    {actionverb} the specified benchmark. This flag may be specified multiple times.

*--benches*::
    {actionverb} all targets in benchmark mode that have the `bench = true`
    manifest flag set. By default this includes the library and binaries built
    as benchmarks, and bench targets. Be aware that this will also build any
    required dependencies, so the lib target may be built twice (once as a
    benchmark, and once as a dependency for binaries, benchmarks, etc.).
    Targets may be enabled or disabled by setting the `bench` flag in the
    manifest settings for the target.

*--all-targets*::
    {actionverb} all targets. This is equivalent to specifying `--lib --bins
    --tests --benches --examples`.
