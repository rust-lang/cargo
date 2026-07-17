# cargo-new(1)

## NAME

cargo-new --- Create a new Cargo package

## SYNOPSIS

`cargo new` [_options_] _path_

## DESCRIPTION

This command will create a new Cargo package in the given directory. This
includes a simple template with a `Cargo.toml` manifest, sample source file,
and a VCS ignore file. If the directory is not already in a VCS repository,
then a new repository is created (see `--vcs` below).

See [cargo-init(1)](cargo-init.html) for a similar command which will create a new manifest
in an existing directory.

## OPTIONS

### New Options

<dl>

<dt class="option-term" id="option-cargo-new---bin"><a class="option-anchor" href="#option-cargo-new---bin"><code>--bin</code></a></dt>
<dd class="option-desc"><p>Create a package with a binary target (<code>src/main.rs</code>).
This is the default behavior.</p>
</dd>


<dt class="option-term" id="option-cargo-new---lib"><a class="option-anchor" href="#option-cargo-new---lib"><code>--lib</code></a></dt>
<dd class="option-desc"><p>Create a package with a library target (<code>src/lib.rs</code>).</p>
</dd>


<dt class="option-term" id="option-cargo-new---edition"><a class="option-anchor" href="#option-cargo-new---edition"><code>--edition</code> <em>edition</em></a></dt>
<dd class="option-desc"><p>Specify the Rust edition to use. Default is 2024.
Possible values: 2015, 2018, 2021, 2024</p>
</dd>


<dt class="option-term" id="option-cargo-new---name"><a class="option-anchor" href="#option-cargo-new---name"><code>--name</code> <em>name</em></a></dt>
<dd class="option-desc"><p>Set the package name. Defaults to the directory name.</p>
</dd>


<dt class="option-term" id="option-cargo-new---vcs"><a class="option-anchor" href="#option-cargo-new---vcs"><code>--vcs</code> <em>vcs</em></a></dt>
<dd class="option-desc"><p>Initialize a new VCS repository for the given version control system (git,
hg, pijul, or fossil) or do not initialize any version control at all
(none). If not specified, defaults to <code>git</code> or the configuration value
<code>cargo-new.vcs</code>, or <code>none</code> if already inside a VCS repository.</p>
</dd>


<dt class="option-term" id="option-cargo-new---registry"><a class="option-anchor" href="#option-cargo-new---registry"><code>--registry</code> <em>registry</em></a></dt>
<dd class="option-desc"><p>This sets the <code>publish</code> field in <code>Cargo.toml</code> to the given registry name
which will restrict publishing only to that registry.</p>
<p>Registry names are defined in <a href="../reference/config.html">Cargo config files</a>.
If not specified, the default registry defined by the <code>registry.default</code>
config key is used. If the default registry is not set and <code>--registry</code> is not
used, the <code>publish</code> field will not be set which means that publishing will not
be restricted.</p>
</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-new--v"><a class="option-anchor" href="#option-cargo-new--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-new---verbose"><a class="option-anchor" href="#option-cargo-new---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-new--q"><a class="option-anchor" href="#option-cargo-new--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-new---quiet"><a class="option-anchor" href="#option-cargo-new---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-new---color"><a class="option-anchor" href="#option-cargo-new---color"><code>--color</code> <em>when</em></a></dt>
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

<dt class="option-term" id="option-cargo-new-+toolchain"><a class="option-anchor" href="#option-cargo-new-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-new---config"><a class="option-anchor" href="#option-cargo-new---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-new--C"><a class="option-anchor" href="#option-cargo-new--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-new--h"><a class="option-anchor" href="#option-cargo-new--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-new---help"><a class="option-anchor" href="#option-cargo-new---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-new--Z"><a class="option-anchor" href="#option-cargo-new--Z"><code>-Z</code> <em>flag</em></a></dt>
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

1. Create a binary Cargo package in the given directory:

       cargo new foo

## SEE ALSO
[cargo(1)](cargo.html), [cargo-init(1)](cargo-init.html)
