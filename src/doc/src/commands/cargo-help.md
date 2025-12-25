# cargo-help(1)

## NAME

cargo-help --- Get help for a Cargo command

## SYNOPSIS

`cargo help` [_subcommand_]

## DESCRIPTION

Prints a help message for the given command.

For commands with subcommands, separate the command levels with spaces. For
example, `cargo help report future-incompatibilities` displays help for the
`cargo report future-incompatibilities` command.

Spaces separate hierarchy levels only between a parent command and its
subcommands. Dashes that are part of a command's name, such as
`generate-lockfile`, must always be preserved.

Multiple command levels can also be written as a single dash-joined word.
For example, `cargo help report-future-incompatibilities` is equivalent to
`cargo help report future-incompatibilities`.

## OPTIONS

### Display Options

<dl>
<dt class="option-term" id="option-cargo-help--v"><a class="option-anchor" href="#option-cargo-help--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-help---verbose"><a class="option-anchor" href="#option-cargo-help---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-help--q"><a class="option-anchor" href="#option-cargo-help--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-help---quiet"><a class="option-anchor" href="#option-cargo-help---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-help---color"><a class="option-anchor" href="#option-cargo-help---color"><code>--color</code> <em>when</em></a></dt>
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
<dt class="option-term" id="option-cargo-help---locked"><a class="option-anchor" href="#option-cargo-help---locked"><code>--locked</code></a></dt>
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


<dt class="option-term" id="option-cargo-help---offline"><a class="option-anchor" href="#option-cargo-help---offline"><code>--offline</code></a></dt>
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


<dt class="option-term" id="option-cargo-help---frozen"><a class="option-anchor" href="#option-cargo-help---frozen"><code>--frozen</code></a></dt>
<dd class="option-desc"><p>Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</p>
</dd>

</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-help-+toolchain"><a class="option-anchor" href="#option-cargo-help-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-help---config"><a class="option-anchor" href="#option-cargo-help---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-help--C"><a class="option-anchor" href="#option-cargo-help--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-help--h"><a class="option-anchor" href="#option-cargo-help--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-help---help"><a class="option-anchor" href="#option-cargo-help---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-help--Z"><a class="option-anchor" href="#option-cargo-help--Z"><code>-Z</code> <em>flag</em></a></dt>
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

1. Get help for a command:

       cargo help build

2. Get help for a nested command:

       cargo help report future-incompatibilities

3. The dash-joined form also works:

       cargo help report-future-incompatibilities

4. Help is also available with the `--help` flag:

       cargo build --help

## SEE ALSO
[cargo(1)](cargo.html)
