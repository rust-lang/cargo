# cargo-yank(1)

## NAME

cargo-yank --- Remove a pushed crate from the index

## SYNOPSIS

`cargo yank` [_options_] _crate_@_version_\
`cargo yank` [_options_] `--version` _version_ [_crate_]

## DESCRIPTION

The yank command removes a previously published crate's version from the
server's index. This command does not delete any data, and the crate will
still be available for download via the registry's download link.

Cargo will not use a yanked version for any new project or checkout without a
pre-existing lockfile, and will generate an error if there are no longer
any compatible versions for your crate.

This command requires you to be authenticated with either the `--token` option
or using [cargo-login(1)](cargo-login.html).

If the crate name is not specified, it will use the package name from the
current directory.

### How yank works

For example, the `foo` crate published version `0.22.0` and another crate `bar`
declared a dependency on version `foo = 0.22`. Now `foo` releases a new, but
not semver compatible, version `0.23.0`, and finds a critical issue with `0.22.0`.
If `0.22.0` is yanked, no new project or checkout without an existing lockfile will be
able to use crate `bar` as it relies on `0.22`.

In this case, the maintainers of `foo` should first publish a semver compatible version
such as `0.22.1` prior to yanking `0.22.0` so that `bar` and all projects that depend
on `bar` will continue to work.

As another example, consider a crate `bar` with published versions `0.22.0`, `0.22.1`, 
`0.22.2`, `0.23.0` and `0.24.0`. The following table identifies the versions
cargo could use in the absence of a lockfile for different SemVer requirements,
following a given release being yanked:

| Yanked Version / SemVer requirement | `bar = "0.22.0"`                          | `bar = "=0.22.0"` | `bar = "0.23.0"` |
|-------------------------------------|-------------------------------------------|-------------------|------------------|
| `0.22.0`                            | Use either `0.22.1` or `0.22.2`           | **Return Error**  | Use `0.23.0`     |
| `0.22.1`                            | Use either `0.22.0` or `0.22.2`           | Use `0.22.0`      | Use `0.23.0`     |
| `0.23.0`                            | Use either `0.22.0`, `0.21.0` or `0.22.2` | Use `0.22.0`      | **Return Error** |

### When to yank

Crates should only be yanked in exceptional circumstances, for example,
license/copyright issues, accidental inclusion of
[PII](https://en.wikipedia.org/wiki/Personal_data), credentials, etc...
In the case of security vulnerabilities, [RustSec](https://rustsec.org/) is
typically a less disruptive mechanism to inform users and encourage them to
upgrade, and avoids the possibility of significant downstream disruption
irrespective of susceptibility to the vulnerability in question.

A common workflow is to yank a crate having already published a semver compatible version,
to reduce the probability of preventing dependent crates from compiling.

## OPTIONS

### Yank Options

<dl>

<dt class="option-term" id="option-cargo-yank---vers"><a class="option-anchor" href="#option-cargo-yank---vers"></a><code>--vers</code> <em>version</em></dt>
<dt class="option-term" id="option-cargo-yank---version"><a class="option-anchor" href="#option-cargo-yank---version"></a><code>--version</code> <em>version</em></dt>
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
<dd class="option-desc">Use verbose output. May be specified twice for “very verbose” output which
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


<dt class="option-term" id="option-cargo-yank---config"><a class="option-anchor" href="#option-cargo-yank---config"></a><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></dt>
<dd class="option-desc">Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</dd>


<dt class="option-term" id="option-cargo-yank--C"><a class="option-anchor" href="#option-cargo-yank--C"></a><code>-C</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example.</dd>


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

       cargo yank foo@1.0.7

## SEE ALSO
[cargo(1)](cargo.html), [cargo-login(1)](cargo-login.html), [cargo-publish(1)](cargo-publish.html)
