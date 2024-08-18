# cargo-update(1)

## NAME

cargo-update --- Update dependencies as recorded in the local lock file

## SYNOPSIS

`cargo update` [_options_] _spec_

## DESCRIPTION

This command will update dependencies in the `Cargo.lock` file to the latest
version. If the `Cargo.lock` file does not exist, it will be created with the
latest available versions.

## OPTIONS

### Update Options

<dl>

<dt class="option-term" id="option-cargo-update-spec…"><a class="option-anchor" href="#option-cargo-update-spec…"></a><em>spec</em>…</dt>
<dd class="option-desc">Update only the specified packages. This flag may be specified
multiple times. See <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the SPEC format.</p>
<p>If packages are specified with <em>spec</em>, then a conservative update of
the lockfile will be performed. This means that only the dependency specified
by SPEC will be updated. Its transitive dependencies will be updated only if
SPEC cannot be updated without updating dependencies.  All other dependencies
will remain locked at their currently recorded versions.</p>
<p>If <em>spec</em> is not specified, all dependencies are updated.</dd>


<dt class="option-term" id="option-cargo-update---recursive"><a class="option-anchor" href="#option-cargo-update---recursive"></a><code>--recursive</code></dt>
<dd class="option-desc">When used with <em>spec</em>, dependencies of <em>spec</em> are forced to update as well.
Cannot be used with <code>--precise</code>.</dd>


<dt class="option-term" id="option-cargo-update---precise"><a class="option-anchor" href="#option-cargo-update---precise"></a><code>--precise</code> <em>precise</em></dt>
<dd class="option-desc">When used with <em>spec</em>, allows you to specify a specific version number to set
the package to. If the package comes from a git repository, this can be a git
revision (such as a SHA hash or tag).</p>
<p>While not recommended, you can specify a yanked version of a package.
When possible, try other non-yanked SemVer-compatible versions or seek help
from the maintainers of the package.</p>
<p>A compatible <code>pre-release</code> version can also be specified even when the version
requirement in <code>Cargo.toml</code> doesn’t contain any pre-release identifier (nightly only).</dd>


<dt class="option-term" id="option-cargo-update---breaking"><a class="option-anchor" href="#option-cargo-update---breaking"></a><code>--breaking</code> <em>directory</em></dt>
<dd class="option-desc">Update <em>spec</em> to latest SemVer-breaking version.</p>
<p>Version requirements will be modified to allow this update.</p>
<p>This only applies to dependencies when</p>
<ul>
<li>The package is a dependency of a workspace member</li>
<li>The dependency is not renamed</li>
<li>A SemVer-incompatible version is available</li>
<li>The “SemVer operator” is used (<code>^</code> which is the default)</li>
</ul>
<p>This option is unstable and available only on the
<a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly channel</a>
and requires the <code>-Z unstable-options</code> flag to enable.
See <a href="https://github.com/rust-lang/cargo/issues/12425">https://github.com/rust-lang/cargo/issues/12425</a> for more information.</dd>


<dt class="option-term" id="option-cargo-update--w"><a class="option-anchor" href="#option-cargo-update--w"></a><code>-w</code></dt>
<dt class="option-term" id="option-cargo-update---workspace"><a class="option-anchor" href="#option-cargo-update---workspace"></a><code>--workspace</code></dt>
<dd class="option-desc">Attempt to update only packages defined in the workspace. Other packages
are updated only if they don’t already exist in the lockfile. This
option is useful for updating <code>Cargo.lock</code> after you’ve changed version
numbers in <code>Cargo.toml</code>.</dd>


<dt class="option-term" id="option-cargo-update---dry-run"><a class="option-anchor" href="#option-cargo-update---dry-run"></a><code>--dry-run</code></dt>
<dd class="option-desc">Displays what would be updated, but doesn’t actually write the lockfile.</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-update--v"><a class="option-anchor" href="#option-cargo-update--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-update---verbose"><a class="option-anchor" href="#option-cargo-update---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-update--q"><a class="option-anchor" href="#option-cargo-update--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-update---quiet"><a class="option-anchor" href="#option-cargo-update---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-update---color"><a class="option-anchor" href="#option-cargo-update---color"></a><code>--color</code> <em>when</em></dt>
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

<dt class="option-term" id="option-cargo-update---manifest-path"><a class="option-anchor" href="#option-cargo-update---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>


<dt class="option-term" id="option-cargo-update---ignore-rust-version"><a class="option-anchor" href="#option-cargo-update---ignore-rust-version"></a><code>--ignore-rust-version</code></dt>
<dd class="option-desc">Ignore <code>rust-version</code> specification in packages.</dd>


<dt class="option-term" id="option-cargo-update---locked"><a class="option-anchor" href="#option-cargo-update---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Asserts that the exact same dependencies and versions are used as when the
existing <code>Cargo.lock</code> file was originally generated. Cargo will exit with an
error when either of the following scenarios arises:</p>
<ul>
<li>The lock file is missing.</li>
<li>Cargo attempted to change the lock file due to a different dependency resolution.</li>
</ul>
<p>It may be used in environments where deterministic builds are desired,
such as in CI pipelines.</dd>


<dt class="option-term" id="option-cargo-update---offline"><a class="option-anchor" href="#option-cargo-update---offline"></a><code>--offline</code></dt>
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


<dt class="option-term" id="option-cargo-update---frozen"><a class="option-anchor" href="#option-cargo-update---frozen"></a><code>--frozen</code></dt>
<dd class="option-desc">Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</dd>


<dt class="option-term" id="option-cargo-update---lockfile-path"><a class="option-anchor" href="#option-cargo-update---lockfile-path"></a><code>--lockfile-path</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the path of the lockfile from the default (<code>&lt;workspace_root&gt;/Cargo.lock</code>) to <em>PATH</em>. <em>PATH</em> must end with
<code>Cargo.lock</code> (e.g. <code>--lockfile-path /tmp/temporary-lockfile/Cargo.lock</code>). Note that providing
<code>--lockfile-path</code> will ignore existing lockfile at the default path, and instead will
either use the lockfile from <em>PATH</em>, or write a new lockfile into the provided <em>PATH</em> if it doesn’t exist.
This flag can be used to run most commands in read-only directories, writing lockfile into the provided <em>PATH</em>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/14421">#14421</a>).</dd>


</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-update-+toolchain"><a class="option-anchor" href="#option-cargo-update-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-update---config"><a class="option-anchor" href="#option-cargo-update---config"></a><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></dt>
<dd class="option-desc">Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</dd>


<dt class="option-term" id="option-cargo-update--C"><a class="option-anchor" href="#option-cargo-update--C"></a><code>-C</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</dd>


<dt class="option-term" id="option-cargo-update--h"><a class="option-anchor" href="#option-cargo-update--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-update---help"><a class="option-anchor" href="#option-cargo-update---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-update--Z"><a class="option-anchor" href="#option-cargo-update--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>

## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.

## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.

## EXAMPLES

1. Update all dependencies in the lockfile:

       cargo update

2. Update only specific dependencies:

       cargo update foo bar

3. Set a specific dependency to a specific version:

       cargo update foo --precise 1.2.3

## SEE ALSO
[cargo(1)](cargo.html), [cargo-generate-lockfile(1)](cargo-generate-lockfile.html)
