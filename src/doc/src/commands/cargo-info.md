# cargo-info(1)

## NAME

cargo-info --- Display information about a package in the registry. Default registry is crates.io

## SYNOPSIS

`cargo info` [_options_] _spec_

## DESCRIPTION

This command displays information about a package in the registry. It fetches data from the package's Cargo.toml file
and presents it in a human-readable format.

## OPTIONS

### Info Options

<dl>

<dt class="option-term" id="option-cargo-info-spec"><a class="option-anchor" href="#option-cargo-info-spec"></a><em>spec</em></dt>
<dd class="option-desc">Fetch information about the specified package. The <em>spec</em> can be a package ID, see <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the SPEC
format.
If the specified package is part of the current workspace, information from the local Cargo.toml file will be displayed.
If the <code>Cargo.lock</code> file does not exist, it will be created. If no version is specified, the appropriate version will be
selected based on the Minimum Supported Rust Version (MSRV).</dd>

<dt class="option-term" id="option-cargo-info---index"><a class="option-anchor" href="#option-cargo-info---index"></a><code>--index</code> <em>index</em></dt>
<dd class="option-desc">The URL of the registry index to use.</dd>

<dt class="option-term" id="option-cargo-info---registry"><a class="option-anchor" href="#option-cargo-info---registry"></a><code>--registry</code> <em>registry</em></dt>
<dd class="option-desc">Name of the registry to use. Registry names are defined in <a href="../reference/config.html">Cargo config
files</a>. If not specified, the default registry is used,
which is defined by the <code>registry.default</code> config key which defaults to
<code>crates-io</code>.</dd>

</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-info--v"><a class="option-anchor" href="#option-cargo-info--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-info---verbose"><a class="option-anchor" href="#option-cargo-info---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-info--q"><a class="option-anchor" href="#option-cargo-info--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-info---quiet"><a class="option-anchor" href="#option-cargo-info---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-info---color"><a class="option-anchor" href="#option-cargo-info---color"></a><code>--color</code> <em>when</em></dt>
<dd class="option-desc">Control when colored output is used. Valid values:</p>
<ul>
<li><code>auto</code> (default): Automatically detect if color support is available on the
terminal.</li>
<li><code>always</code>: Always display colors.</li>
<li><code>never</code>: Never display colors.</li>
</ul>
<p>May also be specified with the <code>term.color</code>
<a href="../reference/config.html">config value</a>.</dd>

</dl>

### Manifest Options

<dl>
<dt class="option-term" id="option-cargo-info---locked"><a class="option-anchor" href="#option-cargo-info---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Asserts that the exact same dependencies and versions are used as when the
existing <code>Cargo.lock</code> file was originally generated. Cargo will exit with an
error when either of the following scenarios arises:</p>
<ul>
<li>The lock file is missing.</li>
<li>Cargo attempted to change the lock file due to a different dependency resolution.</li>
</ul>
<p>It may be used in environments where deterministic builds are desired,
such as in CI pipelines.</dd>


<dt class="option-term" id="option-cargo-info---offline"><a class="option-anchor" href="#option-cargo-info---offline"></a><code>--offline</code></dt>
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


<dt class="option-term" id="option-cargo-info---frozen"><a class="option-anchor" href="#option-cargo-info---frozen"></a><code>--frozen</code></dt>
<dd class="option-desc">Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</dd>

</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-info-+toolchain"><a class="option-anchor" href="#option-cargo-info-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-info---config"><a class="option-anchor" href="#option-cargo-info---config"></a><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></dt>
<dd class="option-desc">Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</dd>


<dt class="option-term" id="option-cargo-info--C"><a class="option-anchor" href="#option-cargo-info--C"></a><code>-C</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</dd>


<dt class="option-term" id="option-cargo-info--h"><a class="option-anchor" href="#option-cargo-info--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-info---help"><a class="option-anchor" href="#option-cargo-info---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-info--Z"><a class="option-anchor" href="#option-cargo-info--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>

## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.

## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.

## EXAMPLES

1. Inspect the `serde` package from crates.io:

        cargo info serde
2. Inspect the `serde` package with version `1.0.0`:

        cargo info serde@1.0.0
3. Inspect the `serde` package form the local registry:

        cargo info serde --registry my-registry 

## SEE ALSO

[cargo(1)](cargo.html), [cargo-search(1)](cargo-search.html)
