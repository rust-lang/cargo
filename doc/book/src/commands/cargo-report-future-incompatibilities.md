# cargo-report-future-incompatibilities(1)
## NAME

cargo-report-future-incompatibilities --- Reports any crates which will eventually stop compiling

## SYNOPSIS

`cargo report future-incompatibilities` [_options_]

## DESCRIPTION

Displays a report of future-incompatible warnings that were emitted during
previous builds.
These are warnings for changes that may become hard errors in the future,
causing dependencies to stop building in a future version of rustc.

For more, see the chapter on [Future incompat report](../reference/future-incompat-report.html).

## OPTIONS

<dl>

<dt class="option-term" id="option-cargo-report-future-incompatibilities---id"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---id"><code>--id</code> <em>id</em></a></dt>
<dd class="option-desc"><p>Show the report with the specified Cargo-generated id.
If not specified, shows the most recent report.</p>
</dd>


</dl>

### Package Selection

By default, the package in the current working directory is selected. The `-p`
flag can be used to choose a different package in a workspace.

<dl>

<dt class="option-term" id="option-cargo-report-future-incompatibilities--p"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities--p"><code>-p</code> <em>spec</em></a></dt>
<dt class="option-term" id="option-cargo-report-future-incompatibilities---package"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---package"><code>--package</code> <em>spec</em></a></dt>
<dd class="option-desc"><p>The package to display a report for. See <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the SPEC
format.</p>
</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-report-future-incompatibilities--v"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-report-future-incompatibilities---verbose"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-report-future-incompatibilities--q"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-report-future-incompatibilities---quiet"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-report-future-incompatibilities---color"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---color"><code>--color</code> <em>when</em></a></dt>
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

</dl>

### Manifest Options

<dl>
<dt class="option-term" id="option-cargo-report-future-incompatibilities---locked"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---locked"><code>--locked</code></a></dt>
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


<dt class="option-term" id="option-cargo-report-future-incompatibilities---offline"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---offline"><code>--offline</code></a></dt>
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


<dt class="option-term" id="option-cargo-report-future-incompatibilities---frozen"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---frozen"><code>--frozen</code></a></dt>
<dd class="option-desc"><p>Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</p>
</dd>

</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-report-future-incompatibilities-+toolchain"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-report-future-incompatibilities---config"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-report-future-incompatibilities--C"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-report-future-incompatibilities--h"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-report-future-incompatibilities---help"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-report-future-incompatibilities--Z"><a class="option-anchor" href="#option-cargo-report-future-incompatibilities--Z"><code>-Z</code> <em>flag</em></a></dt>
<dd class="option-desc"><p>Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</p>
</dd>


</dl>

## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.

## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.

## EXAMPLES

1. Display the latest future-incompat report:

       cargo report future-incompat

2. Display the latest future-incompat report for a specific package:

       cargo report future-incompat --package my-dep@0.0.1

## SEE ALSO

[cargo(1)](cargo.html), [cargo-report(1)](cargo-report.html), [cargo-build(1)](cargo-build.html)
