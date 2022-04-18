# cargo-add(1)



## NAME

cargo-add - Add dependencies to a Cargo.toml manifest file

## SYNOPSIS

`cargo add` [_options_] _crate_...\
`cargo add` [_options_] `--path` _path_\
`cargo add` [_options_] `--git` _url_ [_crate_...]\


## DESCRIPTION

This command can add or modify dependencies.

The source for the dependency can be specified with:

* _crate_`@`_version_: Fetch from a registry with a version constraint of "_version_"
* `--path` _path_: Fetch from the specified _path_
* `--git` _url_: Pull from a git repo at _url_

If no source is specified, then a best effort will be made to select one, including:

* Existing dependencies in other tables (like `dev-dependencies`)
* Workspace members
* Latest release in the registry

When you add a package that is already present, the existing entry will be updated with the flags specified.

## OPTIONS

### Source options

<dl>

<dt class="option-term" id="option-cargo-add---git"><a class="option-anchor" href="#option-cargo-add---git"></a><code>--git</code> <em>url</em></dt>
<dd class="option-desc"><a href="../reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories">Git URL to add the specified crate from</a>.</dd>


<dt class="option-term" id="option-cargo-add---branch"><a class="option-anchor" href="#option-cargo-add---branch"></a><code>--branch</code> <em>branch</em></dt>
<dd class="option-desc">Branch to use when adding from git.</dd>


<dt class="option-term" id="option-cargo-add---tag"><a class="option-anchor" href="#option-cargo-add---tag"></a><code>--tag</code> <em>tag</em></dt>
<dd class="option-desc">Tag to use when adding from git.</dd>


<dt class="option-term" id="option-cargo-add---rev"><a class="option-anchor" href="#option-cargo-add---rev"></a><code>--rev</code> <em>sha</em></dt>
<dd class="option-desc">Specific commit to use when adding from git.</dd>


<dt class="option-term" id="option-cargo-add---path"><a class="option-anchor" href="#option-cargo-add---path"></a><code>--path</code> <em>path</em></dt>
<dd class="option-desc"><a href="../reference/specifying-dependencies.html#specifying-path-dependencies">Filesystem path</a> to local crate to add.</dd>


<dt class="option-term" id="option-cargo-add---registry"><a class="option-anchor" href="#option-cargo-add---registry"></a><code>--registry</code> <em>registry</em></dt>
<dd class="option-desc">Name of the registry to use. Registry names are defined in <a href="../reference/config.html">Cargo config
files</a>. If not specified, the default registry is used,
which is defined by the <code>registry.default</code> config key which defaults to
<code>crates-io</code>.</dd>



</dl>

### Section options

<dl>

<dt class="option-term" id="option-cargo-add---dev"><a class="option-anchor" href="#option-cargo-add---dev"></a><code>--dev</code></dt>
<dd class="option-desc">Add as a <a href="../reference/specifying-dependencies.html#development-dependencies">development dependency</a>.</dd>


<dt class="option-term" id="option-cargo-add---build"><a class="option-anchor" href="#option-cargo-add---build"></a><code>--build</code></dt>
<dd class="option-desc">Add as a <a href="../reference/specifying-dependencies.html#build-dependencies">build dependency</a>.</dd>


<dt class="option-term" id="option-cargo-add---target"><a class="option-anchor" href="#option-cargo-add---target"></a><code>--target</code> <em>target</em></dt>
<dd class="option-desc">Add as a dependency to the <a href="../reference/specifying-dependencies.html#platform-specific-dependencies">given target platform</a>.</dd>


</dl>


</dl>

### Dependency options

<dl>

<dt class="option-term" id="option-cargo-add---rename"><a class="option-anchor" href="#option-cargo-add---rename"></a><code>--rename</code> <em>name</em></dt>
<dd class="option-desc"><a href="../reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml">Rename</a> the dependency.</dd>


<dt class="option-term" id="option-cargo-add---optional"><a class="option-anchor" href="#option-cargo-add---optional"></a><code>--optional</code></dt>
<dd class="option-desc">Mark the dependency as <a href="../reference/features.html#optional-dependencies">optional</a>.</dd>


<dt class="option-term" id="option-cargo-add---no-optional"><a class="option-anchor" href="#option-cargo-add---no-optional"></a><code>--no-optional</code></dt>
<dd class="option-desc">Mark the dependency as <a href="../reference/features.html#optional-dependencies">required</a>.</dd>


<dt class="option-term" id="option-cargo-add---no-default-features"><a class="option-anchor" href="#option-cargo-add---no-default-features"></a><code>--no-default-features</code></dt>
<dd class="option-desc">Disable the <a href="../reference/features.html#dependency-features">default features</a>.</dd>


<dt class="option-term" id="option-cargo-add---default-features"><a class="option-anchor" href="#option-cargo-add---default-features"></a><code>--default-features</code></dt>
<dd class="option-desc">Re-enable the <a href="../reference/features.html#dependency-features">default features</a>.</dd>


<dt class="option-term" id="option-cargo-add---features"><a class="option-anchor" href="#option-cargo-add---features"></a><code>--features</code> <em>features</em></dt>
<dd class="option-desc">Space or comma separated list of <a href="../reference/features.html#dependency-features">features to
activate</a>. When adding multiple
crates, the features for a specific crate may be enabled with
<code>package-name/feature-name</code> syntax. This flag may be specified multiple times,
which enables all specified features.</dd>


</dl>


### Display Options

<dl>
<dt class="option-term" id="option-cargo-add--v"><a class="option-anchor" href="#option-cargo-add--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-add---verbose"><a class="option-anchor" href="#option-cargo-add---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for &quot;very verbose&quot; output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-add--q"><a class="option-anchor" href="#option-cargo-add--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-add---quiet"><a class="option-anchor" href="#option-cargo-add---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-add---color"><a class="option-anchor" href="#option-cargo-add---color"></a><code>--color</code> <em>when</em></dt>
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
<dt class="option-term" id="option-cargo-add---manifest-path"><a class="option-anchor" href="#option-cargo-add---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>


</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-add-+toolchain"><a class="option-anchor" href="#option-cargo-add-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-add--h"><a class="option-anchor" href="#option-cargo-add--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-add---help"><a class="option-anchor" href="#option-cargo-add---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-add--Z"><a class="option-anchor" href="#option-cargo-add--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>


## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.


## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.


## EXAMPLES

1. Add `regex` as a dependency

       cargo add regex

2. Add `trybuild` as a dev-dependency

       cargo add --dev trybuild

3. Add an older version of `nom` as a dependency

       cargo add nom@5

4. Add support for serializing data structures to json with `derive`s

       cargo add serde serde_json -F serde/derive

## SEE ALSO
[cargo(1)](cargo.html)
