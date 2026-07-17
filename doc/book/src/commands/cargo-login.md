# cargo-login(1)

## NAME

cargo-login --- Log in to a registry

## SYNOPSIS

`cargo login` [_options_] [`--` _args_]

## DESCRIPTION

This command will run a credential provider to save a token so that commands
that require authentication, such as [cargo-publish(1)](cargo-publish.html), will be
automatically authenticated.

All the arguments following the two dashes (`--`) are passed to the credential provider.

For the default `cargo:token` credential provider, the token is saved
in `$CARGO_HOME/credentials.toml`. `CARGO_HOME` defaults to `.cargo`
in your home directory.

If a registry has a credential-provider specified, it will be used. Otherwise,
the providers from the config value `registry.global-credential-providers` will
be attempted, starting from the end of the list.

The _token_ will be read from stdin.

The API token for crates.io may be retrieved from <https://crates.io/me>.

Take care to keep the token secret, it should not be shared with anyone else.

## OPTIONS

### Login Options

<dl>
<dt class="option-term" id="option-cargo-login---registry"><a class="option-anchor" href="#option-cargo-login---registry"><code>--registry</code> <em>registry</em></a></dt>
<dd class="option-desc"><p>Name of the registry to use. Registry names are defined in <a href="../reference/config.html">Cargo config
files</a>. If not specified, the default registry is used,
which is defined by the <code>registry.default</code> config key which defaults to
<code>crates-io</code>.</p>
</dd>

</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-login--v"><a class="option-anchor" href="#option-cargo-login--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-login---verbose"><a class="option-anchor" href="#option-cargo-login---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-login--q"><a class="option-anchor" href="#option-cargo-login--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-login---quiet"><a class="option-anchor" href="#option-cargo-login---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-login---color"><a class="option-anchor" href="#option-cargo-login---color"><code>--color</code> <em>when</em></a></dt>
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

<dt class="option-term" id="option-cargo-login-+toolchain"><a class="option-anchor" href="#option-cargo-login-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-login---config"><a class="option-anchor" href="#option-cargo-login---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-login--C"><a class="option-anchor" href="#option-cargo-login--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-login--h"><a class="option-anchor" href="#option-cargo-login--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-login---help"><a class="option-anchor" href="#option-cargo-login---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-login--Z"><a class="option-anchor" href="#option-cargo-login--Z"><code>-Z</code> <em>flag</em></a></dt>
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

1. Save the token for the default registry:

       cargo login

2. Save the token for a specific registry:

       cargo login --registry my-registry

## SEE ALSO
[cargo(1)](cargo.html), [cargo-logout(1)](cargo-logout.html), [cargo-publish(1)](cargo-publish.html)
