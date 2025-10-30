# cargo-owner(1)

## NAME

cargo-owner --- Manage the owners of a crate on the registry

## SYNOPSIS

`cargo owner` [_options_] `--add` _login_ [_crate_]\
`cargo owner` [_options_] `--remove` _login_ [_crate_]\
`cargo owner` [_options_] `--list` [_crate_]

## DESCRIPTION

This command will modify the owners for a crate on the registry. Owners of a
crate can upload new versions and yank old versions. Non-team owners can also
modify the set of owners, so take care!

This command requires you to be authenticated with either the `--token` option
or using [cargo-login(1)](cargo-login.html).

If the crate name is not specified, it will use the package name from the
current directory.

See [the reference](../reference/publishing.html#cargo-owner) for more
information about owners and publishing.

## OPTIONS

### Owner Options

<dl>

<dt class="option-term" id="option-cargo-owner--a"><a class="option-anchor" href="#option-cargo-owner--a"><code>-a</code></a></dt>
<dt class="option-term" id="option-cargo-owner---add"><a class="option-anchor" href="#option-cargo-owner---add"><code>--add</code> <em>login</em>…</a></dt>
<dd class="option-desc"><p>Invite the given user or team as an owner.</p>
</dd>


<dt class="option-term" id="option-cargo-owner--r"><a class="option-anchor" href="#option-cargo-owner--r"><code>-r</code></a></dt>
<dt class="option-term" id="option-cargo-owner---remove"><a class="option-anchor" href="#option-cargo-owner---remove"><code>--remove</code> <em>login</em>…</a></dt>
<dd class="option-desc"><p>Remove the given user or team as an owner.</p>
</dd>


<dt class="option-term" id="option-cargo-owner--l"><a class="option-anchor" href="#option-cargo-owner--l"><code>-l</code></a></dt>
<dt class="option-term" id="option-cargo-owner---list"><a class="option-anchor" href="#option-cargo-owner---list"><code>--list</code></a></dt>
<dd class="option-desc"><p>List owners of a crate.</p>
</dd>


<dt class="option-term" id="option-cargo-owner---token"><a class="option-anchor" href="#option-cargo-owner---token"><code>--token</code> <em>token</em></a></dt>
<dd class="option-desc"><p>API token to use when authenticating. This overrides the token stored in
the credentials file (which is created by <a href="cargo-login.html">cargo-login(1)</a>).</p>
<p><a href="../reference/config.html">Cargo config</a> environment variables can be
used to override the tokens stored in the credentials file. The token for
crates.io may be specified with the <code>CARGO_REGISTRY_TOKEN</code> environment
variable. Tokens for other registries may be specified with environment
variables of the form <code>CARGO_REGISTRIES_NAME_TOKEN</code> where <code>NAME</code> is the name
of the registry in all capital letters.</p>
</dd>


<dt class="option-term" id="option-cargo-owner---index"><a class="option-anchor" href="#option-cargo-owner---index"><code>--index</code> <em>index</em></a></dt>
<dd class="option-desc"><p>The URL of the registry index to use.</p>
</dd>


<dt class="option-term" id="option-cargo-owner---registry"><a class="option-anchor" href="#option-cargo-owner---registry"><code>--registry</code> <em>registry</em></a></dt>
<dd class="option-desc"><p>Name of the registry to use. Registry names are defined in <a href="../reference/config.html">Cargo config
files</a>. If not specified, the default registry is used,
which is defined by the <code>registry.default</code> config key which defaults to
<code>crates-io</code>.</p>
</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-owner--v"><a class="option-anchor" href="#option-cargo-owner--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-owner---verbose"><a class="option-anchor" href="#option-cargo-owner---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-owner--q"><a class="option-anchor" href="#option-cargo-owner--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-owner---quiet"><a class="option-anchor" href="#option-cargo-owner---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-owner---color"><a class="option-anchor" href="#option-cargo-owner---color"><code>--color</code> <em>when</em></a></dt>
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

<dt class="option-term" id="option-cargo-owner-+toolchain"><a class="option-anchor" href="#option-cargo-owner-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-owner---config"><a class="option-anchor" href="#option-cargo-owner---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-owner--C"><a class="option-anchor" href="#option-cargo-owner--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-owner--h"><a class="option-anchor" href="#option-cargo-owner--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-owner---help"><a class="option-anchor" href="#option-cargo-owner---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-owner--Z"><a class="option-anchor" href="#option-cargo-owner--Z"><code>-Z</code> <em>flag</em></a></dt>
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

1. List owners of a package:

       cargo owner --list foo

2. Invite an owner to a package:

       cargo owner --add username foo

3. Remove an owner from a package:

       cargo owner --remove username foo

## SEE ALSO
[cargo(1)](cargo.html), [cargo-login(1)](cargo-login.html), [cargo-publish(1)](cargo-publish.html)
