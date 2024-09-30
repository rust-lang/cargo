# cargo-clean(1)
## NAME

cargo-clean --- Remove generated artifacts

## SYNOPSIS

`cargo clean` [_options_]

## DESCRIPTION

Remove artifacts from the target directory that Cargo has generated in the
past.

With no options, `cargo clean` will delete the entire target directory.

## OPTIONS

### Package Selection

When no packages are selected, all packages and all dependencies in the
workspace are cleaned.

<dl>
<dt class="option-term" id="option-cargo-clean--p"><a class="option-anchor" href="#option-cargo-clean--p"></a><code>-p</code> <em>spec</em>…</dt>
<dt class="option-term" id="option-cargo-clean---package"><a class="option-anchor" href="#option-cargo-clean---package"></a><code>--package</code> <em>spec</em>…</dt>
<dd class="option-desc">Clean only the specified packages. This flag may be specified
multiple times. See <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the SPEC format.</dd>

</dl>

### Clean Options

<dl>

<dt class="option-term" id="option-cargo-clean---dry-run"><a class="option-anchor" href="#option-cargo-clean---dry-run"></a><code>--dry-run</code></dt>
<dd class="option-desc">Displays a summary of what would be deleted without deleting anything.
Use with <code>--verbose</code> to display the actual files that would be deleted.</dd>


<dt class="option-term" id="option-cargo-clean---doc"><a class="option-anchor" href="#option-cargo-clean---doc"></a><code>--doc</code></dt>
<dd class="option-desc">This option will cause <code>cargo clean</code> to remove only the <code>doc</code> directory in
the target directory.</dd>


<dt class="option-term" id="option-cargo-clean---release"><a class="option-anchor" href="#option-cargo-clean---release"></a><code>--release</code></dt>
<dd class="option-desc">Remove all artifacts in the <code>release</code> directory.</dd>


<dt class="option-term" id="option-cargo-clean---profile"><a class="option-anchor" href="#option-cargo-clean---profile"></a><code>--profile</code> <em>name</em></dt>
<dd class="option-desc">Remove all artifacts in the directory with the given profile name.</dd>


<dt class="option-term" id="option-cargo-clean---target-dir"><a class="option-anchor" href="#option-cargo-clean---target-dir"></a><code>--target-dir</code> <em>directory</em></dt>
<dd class="option-desc">Directory for all generated artifacts and intermediate files. May also be
specified with the <code>CARGO_TARGET_DIR</code> environment variable, or the
<code>build.target-dir</code> <a href="../reference/config.html">config value</a>.
Defaults to <code>target</code> in the root of the workspace.</dd>


<dt class="option-term" id="option-cargo-clean---target"><a class="option-anchor" href="#option-cargo-clean---target"></a><code>--target</code> <em>triple</em></dt>
<dd class="option-desc">Clean for the given architecture. The default is the host architecture. The general format of the triple is
<code>&lt;arch&gt;&lt;sub&gt;-&lt;vendor&gt;-&lt;sys&gt;-&lt;abi&gt;</code>. Run <code>rustc --print target-list</code> for a
list of supported targets. This flag may be specified multiple times.</p>
<p>This may also be specified with the <code>build.target</code>
<a href="../reference/config.html">config value</a>.</p>
<p>Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
<a href="../reference/build-cache.html">build cache</a> documentation for more details.</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-clean--v"><a class="option-anchor" href="#option-cargo-clean--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-clean---verbose"><a class="option-anchor" href="#option-cargo-clean---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-clean--q"><a class="option-anchor" href="#option-cargo-clean--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-clean---quiet"><a class="option-anchor" href="#option-cargo-clean---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-clean---color"><a class="option-anchor" href="#option-cargo-clean---color"></a><code>--color</code> <em>when</em></dt>
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
<dt class="option-term" id="option-cargo-clean---manifest-path"><a class="option-anchor" href="#option-cargo-clean---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>


<dt class="option-term" id="option-cargo-clean---locked"><a class="option-anchor" href="#option-cargo-clean---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Asserts that the exact same dependencies and versions are used as when the
existing <code>Cargo.lock</code> file was originally generated. Cargo will exit with an
error when either of the following scenarios arises:</p>
<ul>
<li>The lock file is missing.</li>
<li>Cargo attempted to change the lock file due to a different dependency resolution.</li>
</ul>
<p>It may be used in environments where deterministic builds are desired,
such as in CI pipelines.</dd>


<dt class="option-term" id="option-cargo-clean---offline"><a class="option-anchor" href="#option-cargo-clean---offline"></a><code>--offline</code></dt>
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


<dt class="option-term" id="option-cargo-clean---frozen"><a class="option-anchor" href="#option-cargo-clean---frozen"></a><code>--frozen</code></dt>
<dd class="option-desc">Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</dd>


<dt class="option-term" id="option-cargo-clean---lockfile-path"><a class="option-anchor" href="#option-cargo-clean---lockfile-path"></a><code>--lockfile-path</code> <em>PATH</em></dt>
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

<dt class="option-term" id="option-cargo-clean-+toolchain"><a class="option-anchor" href="#option-cargo-clean-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-clean---config"><a class="option-anchor" href="#option-cargo-clean---config"></a><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></dt>
<dd class="option-desc">Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</dd>


<dt class="option-term" id="option-cargo-clean--C"><a class="option-anchor" href="#option-cargo-clean--C"></a><code>-C</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</dd>


<dt class="option-term" id="option-cargo-clean--h"><a class="option-anchor" href="#option-cargo-clean--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-clean---help"><a class="option-anchor" href="#option-cargo-clean---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-clean--Z"><a class="option-anchor" href="#option-cargo-clean--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>

## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.

## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.

## EXAMPLES

1. Remove the entire target directory:

       cargo clean

2. Remove only the release artifacts:

       cargo clean --release

## SEE ALSO
[cargo(1)](cargo.html), [cargo-build(1)](cargo-build.html)
