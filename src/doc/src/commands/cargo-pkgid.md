# cargo-pkgid(1)

## NAME

cargo-pkgid --- Print a fully qualified package specification

## SYNOPSIS

`cargo pkgid` [_options_] [_spec_]

## DESCRIPTION

Given a _spec_ argument, print out the fully qualified package ID specifier
for a package or dependency in the current workspace. This command will
generate an error if _spec_ is ambiguous as to which package it refers to in
the dependency graph. If no _spec_ is given, then the specifier for the local
package is printed.

This command requires that a lockfile is available and dependencies have been
fetched.

A package specifier consists of a name, version, and source URL. You are
allowed to use partial specifiers to succinctly match a specific package as
long as it matches only one package. This specifier is also used by other parts
in Cargo, such as [cargo-metadata(1)](cargo-metadata.html) and [JSON messages] emitted by Cargo.

The format of a _spec_ can be one of the following:

SPEC Structure             | Example SPEC
---------------------------|--------------
_name_                     | `bitflags`
_name_`@`_version_         | `bitflags@1.0.4`
_url_                      | `https://github.com/rust-lang/cargo`
_url_`#`_version_          | `https://github.com/rust-lang/cargo#0.33.0`
_url_`#`_name_             | `https://github.com/rust-lang/crates.io-index#bitflags`
_url_`#`_name_`@`_version_ | `https://github.com/rust-lang/cargo#crates-io@0.21.0`

The specification grammar can be found in chapter [Package ID Specifications].

## OPTIONS

### Package Selection

<dl>

<dt class="option-term" id="option-cargo-pkgid--p"><a class="option-anchor" href="#option-cargo-pkgid--p"><code>-p</code> <em>spec</em></a></dt>
<dt class="option-term" id="option-cargo-pkgid---package"><a class="option-anchor" href="#option-cargo-pkgid---package"><code>--package</code> <em>spec</em></a></dt>
<dd class="option-desc"><p>Get the package ID for the given package instead of the current package.</p>
</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-pkgid--v"><a class="option-anchor" href="#option-cargo-pkgid--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-pkgid---verbose"><a class="option-anchor" href="#option-cargo-pkgid---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-pkgid--q"><a class="option-anchor" href="#option-cargo-pkgid--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-pkgid---quiet"><a class="option-anchor" href="#option-cargo-pkgid---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-pkgid---color"><a class="option-anchor" href="#option-cargo-pkgid---color"><code>--color</code> <em>when</em></a></dt>
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

<dt class="option-term" id="option-cargo-pkgid---manifest-path"><a class="option-anchor" href="#option-cargo-pkgid---manifest-path"><code>--manifest-path</code> <em>path</em></a></dt>
<dd class="option-desc"><p>Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</p>
</dd>


<dt class="option-term" id="option-cargo-pkgid---locked"><a class="option-anchor" href="#option-cargo-pkgid---locked"><code>--locked</code></a></dt>
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


<dt class="option-term" id="option-cargo-pkgid---offline"><a class="option-anchor" href="#option-cargo-pkgid---offline"><code>--offline</code></a></dt>
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


<dt class="option-term" id="option-cargo-pkgid---frozen"><a class="option-anchor" href="#option-cargo-pkgid---frozen"><code>--frozen</code></a></dt>
<dd class="option-desc"><p>Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</p>
</dd>


<dt class="option-term" id="option-cargo-pkgid---lockfile-path"><a class="option-anchor" href="#option-cargo-pkgid---lockfile-path"><code>--lockfile-path</code> <em>PATH</em></a></dt>
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

<dt class="option-term" id="option-cargo-pkgid-+toolchain"><a class="option-anchor" href="#option-cargo-pkgid-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-pkgid---config"><a class="option-anchor" href="#option-cargo-pkgid---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-pkgid--C"><a class="option-anchor" href="#option-cargo-pkgid--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-pkgid--h"><a class="option-anchor" href="#option-cargo-pkgid--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-pkgid---help"><a class="option-anchor" href="#option-cargo-pkgid---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-pkgid--Z"><a class="option-anchor" href="#option-cargo-pkgid--Z"><code>-Z</code> <em>flag</em></a></dt>
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

1. Retrieve package specification for `foo` package:

       cargo pkgid foo

2. Retrieve package specification for version 1.0.0 of `foo`:

       cargo pkgid foo@1.0.0

3. Retrieve package specification for `foo` from crates.io:

       cargo pkgid https://github.com/rust-lang/crates.io-index#foo

4. Retrieve package specification for `foo` from a local package:

       cargo pkgid file:///path/to/local/package#foo

## SEE ALSO

[cargo(1)](cargo.html), [cargo-generate-lockfile(1)](cargo-generate-lockfile.html), [cargo-metadata(1)](cargo-metadata.html),
[Package ID Specifications], [JSON messages]

[Package ID Specifications]: ../reference/pkgid-spec.html
[JSON messages]: ../reference/external-tools.html#json-messages
