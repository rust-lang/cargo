# cargo-yank(1)

## NAME

cargo-yank - Remove a pushed crate from the index

## SYNOPSIS

`cargo yank` [_options_] `--vers` _version_ [_crate_]

## DESCRIPTION

The yank command removes a previously published crate's version from the
server's index. This command does not delete any data, and the crate will
still be available for download via the registry's download link.

Note that existing crates locked to a yanked version will still be able to
download the yanked version to use it. Cargo will, however, not allow any new
crates to be locked to any yanked version.

This command requires you to be authenticated with either the `--token` option
or using [cargo-login(1)](cargo-login.html).

If the crate name is not specified, it will use the package name from the
current directory.

## OPTIONS

### Yank Options

<dl>

<dt class="option-term" id="option-cargo-yank---vers"><a class="option-anchor" href="#option-cargo-yank---vers"></a><code>--vers</code> <em>version</em></dt>
<dd class="option-desc">The version to yank or un-yank.</dd>


<dt class="option-term" id="option-cargo-yank---undo"><a class="option-anchor" href="#option-cargo-yank---undo"></a><code>--undo</code></dt>
<dd class="option-desc">Undo a yank, putting a version back into the index.</dd>


<dt class="option-term" id="option-cargo-yank---token"><a class="option-anchor" href="#option-cargo-yank---token"></a><code>--token</code> <em>token</em></dt>
<dd class="option-desc">API token to use when authenticating. This overrides the token stored in
the credentials file (which is created by <a href="cargo-login.html">cargo-login(1)</a>).</p>
<p><a href="../reference/config.html">Cargo config</a> environment variables can be
used to override the tokens stored in the credentials file. The token for
crates.io may be specified with the <code>CARGO_REGISTRY_TOKEN</code> environment
variable. Tokens for other registries may be specified with environment
variables of the form <code>CARGO_REGISTRIES_NAME_TOKEN</code> where <code>NAME</code> is the name
of the registry in all capital letters.</dd>



<dt class="option-term" id="option-cargo-yank---index"><a class="option-anchor" href="#option-cargo-yank---index"></a><code>--index</code> <em>index</em></dt>
<dd class="option-desc">The URL of the registry index to use.</dd>



<dt class="option-term" id="option-cargo-yank---registry"><a class="option-anchor" href="#option-cargo-yank---registry"></a><code>--registry</code> <em>registry</em></dt>
<dd class="option-desc">Name of the registry to use. Registry names are defined in <a href="../reference/config.html">Cargo config
files</a>. If not specified, the default registry is used,
which is defined by the <code>registry.default</code> config key which defaults to
<code>crates-io</code>.</dd>



</dl>

### Display Options

<dl>

<dt class="option-term" id="option-cargo-yank--v"><a class="option-anchor" href="#option-cargo-yank--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-yank---verbose"><a class="option-anchor" href="#option-cargo-yank---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for &quot;very verbose&quot; output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-yank--q"><a class="option-anchor" href="#option-cargo-yank--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-yank---quiet"><a class="option-anchor" href="#option-cargo-yank---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-yank---color"><a class="option-anchor" href="#option-cargo-yank---color"></a><code>--color</code> <em>when</em></dt>
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

### Common Options

<dl>

<dt class="option-term" id="option-cargo-yank-+toolchain"><a class="option-anchor" href="#option-cargo-yank-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-yank--h"><a class="option-anchor" href="#option-cargo-yank--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-yank---help"><a class="option-anchor" href="#option-cargo-yank---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-yank--Z"><a class="option-anchor" href="#option-cargo-yank--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>


## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.


## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.


## EXAMPLES

1. Yank a crate from the index:

       cargo yank --vers 1.0.7 foo

## SEE ALSO
[cargo(1)](cargo.html), [cargo-login(1)](cargo-login.html), [cargo-publish(1)](cargo-publish.html)
