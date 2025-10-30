# cargo-locate-project(1)

## NAME

cargo-locate-project --- Print a JSON representation of a Cargo.toml file's location

## SYNOPSIS

`cargo locate-project` [_options_]

## DESCRIPTION

This command will print a JSON object to stdout with the full path to the manifest. The
manifest is found by searching upward for a file named `Cargo.toml` starting from the current
working directory.

If the project happens to be a part of a workspace, the manifest of the project, rather than
the workspace root, is output. This can be overridden by the `--workspace` flag. The root
workspace is found by traversing further upward or by using the field `package.workspace` after
locating the manifest of a workspace member.

## OPTIONS

<dl>

<dt class="option-term" id="option-cargo-locate-project---workspace"><a class="option-anchor" href="#option-cargo-locate-project---workspace"><code>--workspace</code></a></dt>
<dd class="option-desc"><p>Locate the <code>Cargo.toml</code> at the root of the workspace, as opposed to the current
workspace member.</p>
</dd>


</dl>

### Display Options

<dl>

<dt class="option-term" id="option-cargo-locate-project---message-format"><a class="option-anchor" href="#option-cargo-locate-project---message-format"><code>--message-format</code> <em>fmt</em></a></dt>
<dd class="option-desc"><p>The representation in which to print the project location. Valid values:</p>
<ul>
<li><code>json</code> (default): JSON object with the path under the key “root”.</li>
<li><code>plain</code>: Just the path.</li>
</ul>
</dd>


<dt class="option-term" id="option-cargo-locate-project--v"><a class="option-anchor" href="#option-cargo-locate-project--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-locate-project---verbose"><a class="option-anchor" href="#option-cargo-locate-project---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-locate-project--q"><a class="option-anchor" href="#option-cargo-locate-project--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-locate-project---quiet"><a class="option-anchor" href="#option-cargo-locate-project---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-locate-project---color"><a class="option-anchor" href="#option-cargo-locate-project---color"><code>--color</code> <em>when</em></a></dt>
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
<dt class="option-term" id="option-cargo-locate-project---manifest-path"><a class="option-anchor" href="#option-cargo-locate-project---manifest-path"><code>--manifest-path</code> <em>path</em></a></dt>
<dd class="option-desc"><p>Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</p>
</dd>

</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-locate-project-+toolchain"><a class="option-anchor" href="#option-cargo-locate-project-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-locate-project---config"><a class="option-anchor" href="#option-cargo-locate-project---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-locate-project--C"><a class="option-anchor" href="#option-cargo-locate-project--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-locate-project--h"><a class="option-anchor" href="#option-cargo-locate-project--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-locate-project---help"><a class="option-anchor" href="#option-cargo-locate-project---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-locate-project--Z"><a class="option-anchor" href="#option-cargo-locate-project--Z"><code>-Z</code> <em>flag</em></a></dt>
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

1. Display the path to the manifest based on the current directory:

       cargo locate-project

## SEE ALSO
[cargo(1)](cargo.html), [cargo-metadata(1)](cargo-metadata.html)
