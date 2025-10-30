# cargo-run(1)
## NAME

cargo-run --- Run the current package

## SYNOPSIS

`cargo run` [_options_] [`--` _args_]

## DESCRIPTION

Run a binary or example of the local package.

All the arguments following the two dashes (`--`) are passed to the binary to
run. If you're passing arguments to both Cargo and the binary, the ones after
`--` go to the binary, the ones before go to Cargo.

Unlike [cargo-test(1)](cargo-test.html) and [cargo-bench(1)](cargo-bench.html), `cargo run` sets the 
working directory of the binary executed to the current working directory, same 
as if it was executed in the shell directly.

## OPTIONS

### Package Selection

By default, the package in the current working directory is selected. The `-p`
flag can be used to choose a different package in a workspace.

<dl>

<dt class="option-term" id="option-cargo-run--p"><a class="option-anchor" href="#option-cargo-run--p"><code>-p</code> <em>spec</em></a></dt>
<dt class="option-term" id="option-cargo-run---package"><a class="option-anchor" href="#option-cargo-run---package"><code>--package</code> <em>spec</em></a></dt>
<dd class="option-desc"><p>The package to run. See <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the SPEC
format.</p>
</dd>


</dl>

### Target Selection

When no target selection options are given, `cargo run` will run the binary
target. If there are multiple binary targets, you must pass a target flag to
choose one. Or, the `default-run` field may be specified in the `[package]`
section of `Cargo.toml` to choose the name of the binary to run by default.

<dl>

<dt class="option-term" id="option-cargo-run---bin"><a class="option-anchor" href="#option-cargo-run---bin"><code>--bin</code> <em>name</em></a></dt>
<dd class="option-desc"><p>Run the specified binary.</p>
</dd>


<dt class="option-term" id="option-cargo-run---example"><a class="option-anchor" href="#option-cargo-run---example"><code>--example</code> <em>name</em></a></dt>
<dd class="option-desc"><p>Run the specified example.</p>
</dd>


</dl>

### Feature Selection

The feature flags allow you to control which features are enabled. When no
feature options are given, the `default` feature is activated for every
selected package.

See [the features documentation](../reference/features.html#command-line-feature-options)
for more details.

<dl>

<dt class="option-term" id="option-cargo-run--F"><a class="option-anchor" href="#option-cargo-run--F"><code>-F</code> <em>features</em></a></dt>
<dt class="option-term" id="option-cargo-run---features"><a class="option-anchor" href="#option-cargo-run---features"><code>--features</code> <em>features</em></a></dt>
<dd class="option-desc"><p>Space or comma separated list of features to activate. Features of workspace
members may be enabled with <code>package-name/feature-name</code> syntax. This flag may
be specified multiple times, which enables all specified features.</p>
</dd>


<dt class="option-term" id="option-cargo-run---all-features"><a class="option-anchor" href="#option-cargo-run---all-features"><code>--all-features</code></a></dt>
<dd class="option-desc"><p>Activate all available features of all selected packages.</p>
</dd>


<dt class="option-term" id="option-cargo-run---no-default-features"><a class="option-anchor" href="#option-cargo-run---no-default-features"><code>--no-default-features</code></a></dt>
<dd class="option-desc"><p>Do not activate the <code>default</code> feature of the selected packages.</p>
</dd>


</dl>

### Compilation Options

<dl>

<dt class="option-term" id="option-cargo-run---target"><a class="option-anchor" href="#option-cargo-run---target"><code>--target</code> <em>triple</em></a></dt>
<dd class="option-desc"><p>Run for the specified target architecture. The default is the host architecture. The general format of the triple is
<code>&lt;arch&gt;&lt;sub&gt;-&lt;vendor&gt;-&lt;sys&gt;-&lt;abi&gt;</code>.</p>
<p>Possible values:</p>
<ul>
<li>Any supported target in <code>rustc --print target-list</code>.</li>
<li><code>"host-tuple"</code>, which will internally be substituted by the host’s target. This can be particularly useful if you’re cross-compiling some crates, and don’t want to specify your host’s machine as a target (for instance, an <code>xtask</code> in a shared project that may be worked on by many hosts).</li>
<li>A path to a custom target specification. See <a href="../../rustc/targets/custom.html#custom-target-lookup-path">Custom Target Lookup Path</a> for more information.</li>
</ul>
<p>This may also be specified with the <code>build.target</code> <a href="../reference/config.html">config value</a>.</p>
<p>Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
<a href="../reference/build-cache.html">build cache</a> documentation for more details.</p>
</dd>


<dt class="option-term" id="option-cargo-run--r"><a class="option-anchor" href="#option-cargo-run--r"><code>-r</code></a></dt>
<dt class="option-term" id="option-cargo-run---release"><a class="option-anchor" href="#option-cargo-run---release"><code>--release</code></a></dt>
<dd class="option-desc"><p>Run optimized artifacts with the <code>release</code> profile.
See also the <code>--profile</code> option for choosing a specific profile by name.</p>
</dd>


<dt class="option-term" id="option-cargo-run---profile"><a class="option-anchor" href="#option-cargo-run---profile"><code>--profile</code> <em>name</em></a></dt>
<dd class="option-desc"><p>Run with the given profile.
See <a href="../reference/profiles.html">the reference</a> for more details on profiles.</p>
</dd>


<dt class="option-term" id="option-cargo-run---timings=fmts"><a class="option-anchor" href="#option-cargo-run---timings=fmts"><code>--timings=</code><em>fmts</em></a></dt>
<dd class="option-desc"><p>Output information how long each compilation takes, and track concurrency
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
</ul>
</dd>



</dl>

### Output Options

<dl>
<dt class="option-term" id="option-cargo-run---target-dir"><a class="option-anchor" href="#option-cargo-run---target-dir"><code>--target-dir</code> <em>directory</em></a></dt>
<dd class="option-desc"><p>Directory for all generated artifacts and intermediate files. May also be
specified with the <code>CARGO_TARGET_DIR</code> environment variable, or the
<code>build.target-dir</code> <a href="../reference/config.html">config value</a>.
Defaults to <code>target</code> in the root of the workspace.</p>
</dd>

</dl>

### Display Options

<dl>

<dt class="option-term" id="option-cargo-run--v"><a class="option-anchor" href="#option-cargo-run--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-run---verbose"><a class="option-anchor" href="#option-cargo-run---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-run--q"><a class="option-anchor" href="#option-cargo-run--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-run---quiet"><a class="option-anchor" href="#option-cargo-run---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-run---color"><a class="option-anchor" href="#option-cargo-run---color"><code>--color</code> <em>when</em></a></dt>
<dd class="option-desc"><p>Control when colored output is used. Valid values:</p>
<ul>
<li><code>auto</code> (default): Automatically detect if color support is available on the
terminal.</li>
<li><code>always</code>: Always display colors.</li>
<li><code>never</code>: Never display colors.</li>
</ul>
<p>May also be specified with the <code>term.color</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-run---message-format"><a class="option-anchor" href="#option-cargo-run---message-format"><code>--message-format</code> <em>fmt</em></a></dt>
<dd class="option-desc"><p>The output format for diagnostic messages. Can be specified multiple times
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
</ul>
</dd>


</dl>

### Manifest Options

<dl>

<dt class="option-term" id="option-cargo-run---manifest-path"><a class="option-anchor" href="#option-cargo-run---manifest-path"><code>--manifest-path</code> <em>path</em></a></dt>
<dd class="option-desc"><p>Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</p>
</dd>


<dt class="option-term" id="option-cargo-run---ignore-rust-version"><a class="option-anchor" href="#option-cargo-run---ignore-rust-version"><code>--ignore-rust-version</code></a></dt>
<dd class="option-desc"><p>Ignore <code>rust-version</code> specification in packages.</p>
</dd>


<dt class="option-term" id="option-cargo-run---locked"><a class="option-anchor" href="#option-cargo-run---locked"><code>--locked</code></a></dt>
<dd class="option-desc"><p>Asserts that the exact same dependencies and versions are used as when the
existing <code>Cargo.lock</code> file was originally generated. Cargo will exit with an
error when either of the following scenarios arises:</p>
<ul>
<li>The lock file is missing.</li>
<li>Cargo attempted to change the lock file due to a different dependency resolution.</li>
</ul>
<p>It may be used in environments where deterministic builds are desired,
such as in CI pipelines.</p>
</dd>


<dt class="option-term" id="option-cargo-run---offline"><a class="option-anchor" href="#option-cargo-run---offline"><code>--offline</code></a></dt>
<dd class="option-desc"><p>Prevents Cargo from accessing the network for any reason. Without this
flag, Cargo will stop with an error if it needs to access the network and
the network is not available. With this flag, Cargo will attempt to
proceed without the network if possible.</p>
<p>Beware that this may result in different dependency resolution than online
mode. Cargo will restrict itself to crates that are downloaded locally, even
if there might be a newer version as indicated in the local copy of the index.
See the <a href="cargo-fetch.html">cargo-fetch(1)</a> command to download dependencies before going
offline.</p>
<p>May also be specified with the <code>net.offline</code> <a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-run---frozen"><a class="option-anchor" href="#option-cargo-run---frozen"><code>--frozen</code></a></dt>
<dd class="option-desc"><p>Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</p>
</dd>


<dt class="option-term" id="option-cargo-run---lockfile-path"><a class="option-anchor" href="#option-cargo-run---lockfile-path"><code>--lockfile-path</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the path of the lockfile from the default (<code>&lt;workspace_root&gt;/Cargo.lock</code>) to <em>PATH</em>. <em>PATH</em> must end with
<code>Cargo.lock</code> (e.g. <code>--lockfile-path /tmp/temporary-lockfile/Cargo.lock</code>). Note that providing
<code>--lockfile-path</code> will ignore existing lockfile at the default path, and instead will
either use the lockfile from <em>PATH</em>, or write a new lockfile into the provided <em>PATH</em> if it doesn’t exist.
This flag can be used to run most commands in read-only directories, writing lockfile into the provided <em>PATH</em>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/14421">#14421</a>).</p>
</dd>


</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-run-+toolchain"><a class="option-anchor" href="#option-cargo-run-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-run---config"><a class="option-anchor" href="#option-cargo-run---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-run--C"><a class="option-anchor" href="#option-cargo-run--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-run--h"><a class="option-anchor" href="#option-cargo-run--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-run---help"><a class="option-anchor" href="#option-cargo-run---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-run--Z"><a class="option-anchor" href="#option-cargo-run--Z"><code>-Z</code> <em>flag</em></a></dt>
<dd class="option-desc"><p>Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</p>
</dd>


</dl>

### Miscellaneous Options

<dl>
<dt class="option-term" id="option-cargo-run--j"><a class="option-anchor" href="#option-cargo-run--j"><code>-j</code> <em>N</em></a></dt>
<dt class="option-term" id="option-cargo-run---jobs"><a class="option-anchor" href="#option-cargo-run---jobs"><code>--jobs</code> <em>N</em></a></dt>
<dd class="option-desc"><p>Number of parallel jobs to run. May also be specified with the
<code>build.jobs</code> <a href="../reference/config.html">config value</a>. Defaults to
the number of logical CPUs. If negative, it sets the maximum number of
parallel jobs to the number of logical CPUs plus provided value. If
a string <code>default</code> is provided, it sets the value back to defaults.
Should not be 0.</p>
</dd>

<dt class="option-term" id="option-cargo-run---keep-going"><a class="option-anchor" href="#option-cargo-run---keep-going"><code>--keep-going</code></a></dt>
<dd class="option-desc"><p>Build as many crates in the dependency graph as possible, rather than aborting
the build on the first one that fails to build.</p>
<p>For example if the current package depends on dependencies <code>fails</code> and <code>works</code>,
one of which fails to build, <code>cargo run -j1</code> may or may not build the
one that succeeds (depending on which one of the two builds Cargo picked to run
first), whereas <code>cargo run -j1 --keep-going</code> would definitely run both
builds, even if the one run first fails.</p>
</dd>

</dl>

## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.

## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.

## EXAMPLES

1. Build the local package and run its main target (assuming only one binary):

       cargo run

2. Run an example with extra arguments:

       cargo run --example exname -- --exoption exarg1 exarg2

## SEE ALSO
[cargo(1)](cargo.html), [cargo-build(1)](cargo-build.html)
