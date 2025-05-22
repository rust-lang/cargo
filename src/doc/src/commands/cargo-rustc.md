# cargo-rustc(1)
## NAME

cargo-rustc --- Compile the current package, and pass extra options to the compiler

## SYNOPSIS

`cargo rustc` [_options_] [`--` _args_]

## DESCRIPTION

The specified target for the current package (or package specified by `-p` if
provided) will be compiled along with all of its dependencies. The specified
_args_ will all be passed to the final compiler invocation, not any of the
dependencies. Note that the compiler will still unconditionally receive
arguments such as `-L`, `--extern`, and `--crate-type`, and the specified
_args_ will simply be added to the compiler invocation.

See <https://doc.rust-lang.org/rustc/index.html> for documentation on rustc
flags.

This command requires that only one target is being compiled when additional
arguments are provided. If more than one target is available for the current
package the filters of `--lib`, `--bin`, etc, must be used to select which
target is compiled.

To pass flags to all compiler processes spawned by Cargo, use the `RUSTFLAGS`
[environment variable](../reference/environment-variables.html) or the
`build.rustflags` [config value](../reference/config.html).

## OPTIONS

### Package Selection

By default, the package in the current working directory is selected. The `-p`
flag can be used to choose a different package in a workspace.

<dl>

<dt class="option-term" id="option-cargo-rustc--p"><a class="option-anchor" href="#option-cargo-rustc--p"></a><code>-p</code> <em>spec</em></dt>
<dt class="option-term" id="option-cargo-rustc---package"><a class="option-anchor" href="#option-cargo-rustc---package"></a><code>--package</code> <em>spec</em></dt>
<dd class="option-desc">The package to build. See <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the SPEC
format.</dd>


</dl>

### Target Selection

When no target selection options are given, `cargo rustc` will build all
binary and library targets of the selected package.

Binary targets are automatically built if there is an integration test or
benchmark being selected to build. This allows an integration
test to execute the binary to exercise and test its behavior. 
The `CARGO_BIN_EXE_<name>`
[environment variable](../reference/environment-variables.html#environment-variables-cargo-sets-for-crates)
is set when the integration test is built so that it can use the
[`env` macro](https://doc.rust-lang.org/std/macro.env.html) to locate the
executable.

Passing target selection flags will build only the specified
targets. 

Note that `--bin`, `--example`, `--test` and `--bench` flags also 
support common Unix glob patterns like `*`, `?` and `[]`. However, to avoid your 
shell accidentally expanding glob patterns before Cargo handles them, you must 
use single quotes or double quotes around each glob pattern.

<dl>

<dt class="option-term" id="option-cargo-rustc---lib"><a class="option-anchor" href="#option-cargo-rustc---lib"></a><code>--lib</code></dt>
<dd class="option-desc">Build the package’s library.</dd>


<dt class="option-term" id="option-cargo-rustc---bin"><a class="option-anchor" href="#option-cargo-rustc---bin"></a><code>--bin</code> <em>name</em>…</dt>
<dd class="option-desc">Build the specified binary. This flag may be specified multiple times
and supports common Unix glob patterns.</dd>


<dt class="option-term" id="option-cargo-rustc---bins"><a class="option-anchor" href="#option-cargo-rustc---bins"></a><code>--bins</code></dt>
<dd class="option-desc">Build all binary targets.</dd>


<dt class="option-term" id="option-cargo-rustc---example"><a class="option-anchor" href="#option-cargo-rustc---example"></a><code>--example</code> <em>name</em>…</dt>
<dd class="option-desc">Build the specified example. This flag may be specified multiple times
and supports common Unix glob patterns.</dd>


<dt class="option-term" id="option-cargo-rustc---examples"><a class="option-anchor" href="#option-cargo-rustc---examples"></a><code>--examples</code></dt>
<dd class="option-desc">Build all example targets.</dd>


<dt class="option-term" id="option-cargo-rustc---test"><a class="option-anchor" href="#option-cargo-rustc---test"></a><code>--test</code> <em>name</em>…</dt>
<dd class="option-desc">Build the specified integration test. This flag may be specified
multiple times and supports common Unix glob patterns.</dd>


<dt class="option-term" id="option-cargo-rustc---tests"><a class="option-anchor" href="#option-cargo-rustc---tests"></a><code>--tests</code></dt>
<dd class="option-desc">Build all targets that have the <code>test = true</code> manifest
flag set. By default this includes the library and binaries built as
unittests, and integration tests. Be aware that this will also build any
required dependencies, so the lib target may be built twice (once as a
unittest, and once as a dependency for binaries, integration tests, etc.).
Targets may be enabled or disabled by setting the <code>test</code> flag in the
manifest settings for the target.</dd>


<dt class="option-term" id="option-cargo-rustc---bench"><a class="option-anchor" href="#option-cargo-rustc---bench"></a><code>--bench</code> <em>name</em>…</dt>
<dd class="option-desc">Build the specified benchmark. This flag may be specified multiple
times and supports common Unix glob patterns.</dd>


<dt class="option-term" id="option-cargo-rustc---benches"><a class="option-anchor" href="#option-cargo-rustc---benches"></a><code>--benches</code></dt>
<dd class="option-desc">Build all targets that have the <code>bench = true</code>
manifest flag set. By default this includes the library and binaries built
as benchmarks, and bench targets. Be aware that this will also build any
required dependencies, so the lib target may be built twice (once as a
benchmark, and once as a dependency for binaries, benchmarks, etc.).
Targets may be enabled or disabled by setting the <code>bench</code> flag in the
manifest settings for the target.</dd>


<dt class="option-term" id="option-cargo-rustc---all-targets"><a class="option-anchor" href="#option-cargo-rustc---all-targets"></a><code>--all-targets</code></dt>
<dd class="option-desc">Build all targets. This is equivalent to specifying <code>--lib --bins --tests --benches --examples</code>.</dd>


</dl>

### Feature Selection

The feature flags allow you to control which features are enabled. When no
feature options are given, the `default` feature is activated for every
selected package.

See [the features documentation](../reference/features.html#command-line-feature-options)
for more details.

<dl>

<dt class="option-term" id="option-cargo-rustc--F"><a class="option-anchor" href="#option-cargo-rustc--F"></a><code>-F</code> <em>features</em></dt>
<dt class="option-term" id="option-cargo-rustc---features"><a class="option-anchor" href="#option-cargo-rustc---features"></a><code>--features</code> <em>features</em></dt>
<dd class="option-desc">Space or comma separated list of features to activate. Features of workspace
members may be enabled with <code>package-name/feature-name</code> syntax. This flag may
be specified multiple times, which enables all specified features.</dd>


<dt class="option-term" id="option-cargo-rustc---all-features"><a class="option-anchor" href="#option-cargo-rustc---all-features"></a><code>--all-features</code></dt>
<dd class="option-desc">Activate all available features of all selected packages.</dd>


<dt class="option-term" id="option-cargo-rustc---no-default-features"><a class="option-anchor" href="#option-cargo-rustc---no-default-features"></a><code>--no-default-features</code></dt>
<dd class="option-desc">Do not activate the <code>default</code> feature of the selected packages.</dd>


</dl>

### Compilation Options

<dl>

<dt class="option-term" id="option-cargo-rustc---target"><a class="option-anchor" href="#option-cargo-rustc---target"></a><code>--target</code> <em>triple</em></dt>
<dd class="option-desc">Build for the given architecture. The default is the host architecture. The general format of the triple is
<code>&lt;arch&gt;&lt;sub&gt;-&lt;vendor&gt;-&lt;sys&gt;-&lt;abi&gt;</code>. Run <code>rustc --print target-list</code> for a
list of supported targets. This flag may be specified multiple times.</p>
<p>This may also be specified with the <code>build.target</code>
<a href="../reference/config.html">config value</a>.</p>
<p>Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
<a href="../reference/build-cache.html">build cache</a> documentation for more details.</dd>


<dt class="option-term" id="option-cargo-rustc--r"><a class="option-anchor" href="#option-cargo-rustc--r"></a><code>-r</code></dt>
<dt class="option-term" id="option-cargo-rustc---release"><a class="option-anchor" href="#option-cargo-rustc---release"></a><code>--release</code></dt>
<dd class="option-desc">Build optimized artifacts with the <code>release</code> profile.
See also the <code>--profile</code> option for choosing a specific profile by name.</dd>


<dt class="option-term" id="option-cargo-rustc---profile"><a class="option-anchor" href="#option-cargo-rustc---profile"></a><code>--profile</code> <em>name</em></dt>
<dd class="option-desc">Build with the given profile.</p>
<p>The <code>rustc</code> subcommand will treat the following named profiles with special behaviors:</p>
<ul>
<li><code>check</code> — Builds in the same way as the <a href="cargo-check.html">cargo-check(1)</a> command with
the <code>dev</code> profile.</li>
<li><code>test</code> — Builds in the same way as the <a href="cargo-test.html">cargo-test(1)</a> command,
enabling building in test mode which will enable tests and enable the <code>test</code>
cfg option. See <a href="https://doc.rust-lang.org/rustc/tests/index.html">rustc
tests</a> for more detail.</li>
<li><code>bench</code> — Builds in the same was as the <a href="cargo-bench.html">cargo-bench(1)</a> command,
similar to the <code>test</code> profile.</li>
</ul>
<p>See <a href="../reference/profiles.html">the reference</a> for more details on profiles.</dd>


<dt class="option-term" id="option-cargo-rustc---timings=fmts"><a class="option-anchor" href="#option-cargo-rustc---timings=fmts"></a><code>--timings=</code><em>fmts</em></dt>
<dd class="option-desc">Output information how long each compilation takes, and track concurrency
information over time. Accepts an optional comma-separated list of output
formats; <code>--timings</code> without an argument will default to <code>--timings=html</code>.
Specifying an output format (rather than the default) is unstable and requires
<code>-Zunstable-options</code>. Valid output formats:</p>
<ul>
<li><code>html</code> (unstable, requires <code>-Zunstable-options</code>): Write a human-readable file <code>cargo-timing.html</code> to the
<code>target/cargo-timings</code> directory with a report of the compilation. Also write
a report to the same directory with a timestamp in the filename if you want
to look at older runs. HTML output is suitable for human consumption only,
and does not provide machine-readable timing data.</li>
<li><code>json</code> (unstable, requires <code>-Zunstable-options</code>): Emit machine-readable JSON
information about timing information.</li>
</ul></dd>



<dt class="option-term" id="option-cargo-rustc---crate-type"><a class="option-anchor" href="#option-cargo-rustc---crate-type"></a><code>--crate-type</code> <em>crate-type</em></dt>
<dd class="option-desc">Build for the given crate type. This flag accepts a comma-separated list of
1 or more crate types, of which the allowed values are the same as <code>crate-type</code>
field in the manifest for configuring a Cargo target. See
<a href="../reference/cargo-targets.html#the-crate-type-field"><code>crate-type</code> field</a>
for possible values.</p>
<p>If the manifest contains a list, and <code>--crate-type</code> is provided,
the command-line argument value will override what is in the manifest.</p>
<p>This flag only works when building a <code>lib</code> or <code>example</code> library target.</dd>


</dl>

### Output Options

<dl>
<dt class="option-term" id="option-cargo-rustc---target-dir"><a class="option-anchor" href="#option-cargo-rustc---target-dir"></a><code>--target-dir</code> <em>directory</em></dt>
<dd class="option-desc">Directory for all generated artifacts and intermediate files. May also be
specified with the <code>CARGO_TARGET_DIR</code> environment variable, or the
<code>build.target-dir</code> <a href="../reference/config.html">config value</a>.
Defaults to <code>target</code> in the root of the workspace.</dd>

</dl>

### Display Options

<dl>

<dt class="option-term" id="option-cargo-rustc--v"><a class="option-anchor" href="#option-cargo-rustc--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-rustc---verbose"><a class="option-anchor" href="#option-cargo-rustc---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-rustc--q"><a class="option-anchor" href="#option-cargo-rustc--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-rustc---quiet"><a class="option-anchor" href="#option-cargo-rustc---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-rustc---color"><a class="option-anchor" href="#option-cargo-rustc---color"></a><code>--color</code> <em>when</em></dt>
<dd class="option-desc">Control when colored output is used. Valid values:</p>
<ul>
<li><code>auto</code> (default): Automatically detect if color support is available on the
terminal.</li>
<li><code>always</code>: Always display colors.</li>
<li><code>never</code>: Never display colors.</li>
</ul>
<p>May also be specified with the <code>term.color</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-rustc---message-format"><a class="option-anchor" href="#option-cargo-rustc---message-format"></a><code>--message-format</code> <em>fmt</em></dt>
<dd class="option-desc">The output format for diagnostic messages. Can be specified multiple times
and consists of comma-separated values. Valid values:</p>
<ul>
<li><code>human</code> (default): Display in a human-readable text format. Conflicts with
<code>short</code> and <code>json</code>.</li>
<li><code>short</code>: Emit shorter, human-readable text messages. Conflicts with <code>human</code>
and <code>json</code>.</li>
<li><code>json</code>: Emit JSON messages to stdout. See
<a href="../reference/external-tools.html#json-messages">the reference</a>
for more details. Conflicts with <code>human</code> and <code>short</code>.</li>
<li><code>json-diagnostic-short</code>: Ensure the <code>rendered</code> field of JSON messages contains
the “short” rendering from rustc. Cannot be used with <code>human</code> or <code>short</code>.</li>
<li><code>json-diagnostic-rendered-ansi</code>: Ensure the <code>rendered</code> field of JSON messages
contains embedded ANSI color codes for respecting rustc’s default color
scheme. Cannot be used with <code>human</code> or <code>short</code>.</li>
<li><code>json-render-diagnostics</code>: Instruct Cargo to not include rustc diagnostics
in JSON messages printed, but instead Cargo itself should render the
JSON diagnostics coming from rustc. Cargo’s own JSON diagnostics and others
coming from rustc are still emitted. Cannot be used with <code>human</code> or <code>short</code>.</li>
</ul></dd>


</dl>

### Manifest Options

<dl>

<dt class="option-term" id="option-cargo-rustc---manifest-path"><a class="option-anchor" href="#option-cargo-rustc---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>


<dt class="option-term" id="option-cargo-rustc---ignore-rust-version"><a class="option-anchor" href="#option-cargo-rustc---ignore-rust-version"></a><code>--ignore-rust-version</code></dt>
<dd class="option-desc">Ignore <code>rust-version</code> specification in packages.</dd>


<dt class="option-term" id="option-cargo-rustc---locked"><a class="option-anchor" href="#option-cargo-rustc---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Asserts that the exact same dependencies and versions are used as when the
existing <code>Cargo.lock</code> file was originally generated. Cargo will exit with an
error when either of the following scenarios arises:</p>
<ul>
<li>The lock file is missing.</li>
<li>Cargo attempted to change the lock file due to a different dependency resolution.</li>
</ul>
<p>It may be used in environments where deterministic builds are desired,
such as in CI pipelines.</dd>


<dt class="option-term" id="option-cargo-rustc---offline"><a class="option-anchor" href="#option-cargo-rustc---offline"></a><code>--offline</code></dt>
<dd class="option-desc">Prevents Cargo from accessing the network for any reason. Without this
flag, Cargo will stop with an error if it needs to access the network and
the network is not available. With this flag, Cargo will attempt to
proceed without the network if possible.</p>
<p>Beware that this may result in different dependency resolution than online
mode. Cargo will restrict itself to crates that are downloaded locally, even
if there might be a newer version as indicated in the local copy of the index.
See the <a href="cargo-fetch.html">cargo-fetch(1)</a> command to download dependencies before going
offline.</p>
<p>May also be specified with the <code>net.offline</code> <a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-rustc---frozen"><a class="option-anchor" href="#option-cargo-rustc---frozen"></a><code>--frozen</code></dt>
<dd class="option-desc">Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</dd>


<dt class="option-term" id="option-cargo-rustc---lockfile-path"><a class="option-anchor" href="#option-cargo-rustc---lockfile-path"></a><code>--lockfile-path</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the path of the lockfile from the default (<code>&lt;workspace_root&gt;/Cargo.lock</code>) to <em>PATH</em>. <em>PATH</em> must end with
<code>Cargo.lock</code> (e.g. <code>--lockfile-path /tmp/temporary-lockfile/Cargo.lock</code>). Note that providing
<code>--lockfile-path</code> will ignore existing lockfile at the default path, and instead will
either use the lockfile from <em>PATH</em>, or write a new lockfile into the provided <em>PATH</em> if it doesn’t exist.
This flag can be used to run most commands in read-only directories, writing lockfile into the provided <em>PATH</em>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/14421">#14421</a>).</dd>


</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-rustc-+toolchain"><a class="option-anchor" href="#option-cargo-rustc-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-rustc---config"><a class="option-anchor" href="#option-cargo-rustc---config"></a><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></dt>
<dd class="option-desc">Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</dd>


<dt class="option-term" id="option-cargo-rustc--C"><a class="option-anchor" href="#option-cargo-rustc--C"></a><code>-C</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</dd>


<dt class="option-term" id="option-cargo-rustc--h"><a class="option-anchor" href="#option-cargo-rustc--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-rustc---help"><a class="option-anchor" href="#option-cargo-rustc---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-rustc--Z"><a class="option-anchor" href="#option-cargo-rustc--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>

### Miscellaneous Options

<dl>
<dt class="option-term" id="option-cargo-rustc--j"><a class="option-anchor" href="#option-cargo-rustc--j"></a><code>-j</code> <em>N</em></dt>
<dt class="option-term" id="option-cargo-rustc---jobs"><a class="option-anchor" href="#option-cargo-rustc---jobs"></a><code>--jobs</code> <em>N</em></dt>
<dd class="option-desc">Number of parallel jobs to run. May also be specified with the
<code>build.jobs</code> <a href="../reference/config.html">config value</a>. Defaults to
the number of logical CPUs. If negative, it sets the maximum number of
parallel jobs to the number of logical CPUs plus provided value. If
a string <code>default</code> is provided, it sets the value back to defaults.
Should not be 0.</dd>

<dt class="option-term" id="option-cargo-rustc---keep-going"><a class="option-anchor" href="#option-cargo-rustc---keep-going"></a><code>--keep-going</code></dt>
<dd class="option-desc">Build as many crates in the dependency graph as possible, rather than aborting
the build on the first one that fails to build.</p>
<p>For example if the current package depends on dependencies <code>fails</code> and <code>works</code>,
one of which fails to build, <code>cargo rustc -j1</code> may or may not build the
one that succeeds (depending on which one of the two builds Cargo picked to run
first), whereas <code>cargo rustc -j1 --keep-going</code> would definitely run both
builds, even if the one run first fails.</dd>

<dt class="option-term" id="option-cargo-rustc---future-incompat-report"><a class="option-anchor" href="#option-cargo-rustc---future-incompat-report"></a><code>--future-incompat-report</code></dt>
<dd class="option-desc">Displays a future-incompat report for any future-incompatible warnings
produced during execution of this command</p>
<p>See <a href="cargo-report.html">cargo-report(1)</a></dd>

</dl>

## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.

## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.

## EXAMPLES

1. Check if your package (not including dependencies) uses unsafe code:

       cargo rustc --lib -- -D unsafe-code

2. Try an experimental flag on the nightly compiler, such as this which prints
   the size of every type:

       cargo rustc --lib -- -Z print-type-sizes

3. Override `crate-type` field in Cargo.toml with command-line option:

       cargo rustc --lib --crate-type lib,cdylib

## SEE ALSO
[cargo(1)](cargo.html), [cargo-build(1)](cargo-build.html), [rustc(1)](https://doc.rust-lang.org/rustc/index.html)
