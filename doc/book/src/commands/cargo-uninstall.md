# cargo-uninstall(1)

## NAME

cargo-uninstall --- Remove a Rust binary

## SYNOPSIS

`cargo uninstall` [_options_] [_spec_...]

## DESCRIPTION

This command removes a package installed with [cargo-install(1)](cargo-install.html). The _spec_
argument is a package ID specification of the package to remove (see
[cargo-pkgid(1)](cargo-pkgid.html)).

By default all binaries are removed for a crate but the `--bin` and
`--example` flags can be used to only remove particular binaries.

The installation root is determined, in order of precedence:

- `--root` option
- `CARGO_INSTALL_ROOT` environment variable
- `install.root` Cargo [config value](../reference/config.html)
- `CARGO_HOME` environment variable
- `$HOME/.cargo`

## OPTIONS

### Uninstall Options

<dl>

<dt class="option-term" id="option-cargo-uninstall--p"><a class="option-anchor" href="#option-cargo-uninstall--p"><code>-p</code></a></dt>
<dt class="option-term" id="option-cargo-uninstall---package"><a class="option-anchor" href="#option-cargo-uninstall---package"><code>--package</code> <em>spec</em>…</a></dt>
<dd class="option-desc"><p>Package to uninstall.</p>
</dd>


<dt class="option-term" id="option-cargo-uninstall---bin"><a class="option-anchor" href="#option-cargo-uninstall---bin"><code>--bin</code> <em>name</em>…</a></dt>
<dd class="option-desc"><p>Only uninstall the binary <em>name</em>.</p>
</dd>


<dt class="option-term" id="option-cargo-uninstall---root"><a class="option-anchor" href="#option-cargo-uninstall---root"><code>--root</code> <em>dir</em></a></dt>
<dd class="option-desc"><p>Directory to uninstall packages from.</p>
</dd>


</dl>

### Display Options

<dl>

<dt class="option-term" id="option-cargo-uninstall--v"><a class="option-anchor" href="#option-cargo-uninstall--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-uninstall---verbose"><a class="option-anchor" href="#option-cargo-uninstall---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-uninstall--q"><a class="option-anchor" href="#option-cargo-uninstall--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-uninstall---quiet"><a class="option-anchor" href="#option-cargo-uninstall---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-uninstall---color"><a class="option-anchor" href="#option-cargo-uninstall---color"><code>--color</code> <em>when</em></a></dt>
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

### Common Options

<dl>

<dt class="option-term" id="option-cargo-uninstall-+toolchain"><a class="option-anchor" href="#option-cargo-uninstall-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-uninstall---config"><a class="option-anchor" href="#option-cargo-uninstall---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-uninstall--C"><a class="option-anchor" href="#option-cargo-uninstall--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-uninstall--h"><a class="option-anchor" href="#option-cargo-uninstall--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-uninstall---help"><a class="option-anchor" href="#option-cargo-uninstall---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-uninstall--Z"><a class="option-anchor" href="#option-cargo-uninstall--Z"><code>-Z</code> <em>flag</em></a></dt>
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

1. Uninstall a previously installed package.

       cargo uninstall ripgrep

## SEE ALSO
[cargo(1)](cargo.html), [cargo-install(1)](cargo-install.html)
