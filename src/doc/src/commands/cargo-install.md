# cargo-install(1)

## NAME

cargo-install --- Build and install a Rust binary

## SYNOPSIS

`cargo install` [_options_] _crate_[@_version_]...\
`cargo install` [_options_] `--path` _path_\
`cargo install` [_options_] `--git` _url_ [_crate_...]\
`cargo install` [_options_] `--list`

## DESCRIPTION

This command manages Cargo's local set of installed binary crates. Only
packages which have executable `[[bin]]` or `[[example]]` targets can be
installed, and all executables are installed into the installation root's
`bin` folder. By default only binaries, not examples, are installed.

The installation root is determined, in order of precedence:

- `--root` option
- `CARGO_INSTALL_ROOT` environment variable
- `install.root` Cargo [config value](../reference/config.html)
- `CARGO_HOME` environment variable
- `$HOME/.cargo`


There are multiple sources from which a crate can be installed. The default
location is crates.io but the `--git`, `--path`, and `--registry` flags can
change this source. If the source contains more than one package (such as
crates.io or a git repository with multiple crates) the _crate_ argument is
required to indicate which crate should be installed.

Crates from crates.io can optionally specify the version they wish to install
via the `--version` flags, and similarly packages from git repositories can
optionally specify the branch, tag, or revision that should be installed. If a
crate has multiple binaries, the `--bin` argument can selectively install only
one of them, and if you'd rather install examples the `--example` argument can
be used as well.

If the package is already installed, Cargo will reinstall it if the installed
version does not appear to be up-to-date. If any of the following values
change, then Cargo will reinstall the package:

- The package version and source.
- The set of binary names installed.
- The chosen features.
- The profile (`--profile`).
- The target (`--target`).

Installing with `--path` will always build and install, unless there are
conflicting binaries from another package. The `--force` flag may be used to
force Cargo to always reinstall the package.

If the source is crates.io or `--git` then by default the crate will be built
in a temporary target directory. To avoid this, the target directory can be
specified by setting the `CARGO_TARGET_DIR` environment variable to a relative
path. In particular, this can be useful for caching build artifacts on
continuous integration systems.

### Dealing with the Lockfile

By default, the `Cargo.lock` file that is included with the package will be
ignored. This means that Cargo will recompute which versions of dependencies
to use, possibly using newer versions that have been released since the
package was published. The `--locked` flag can be used to force Cargo to use
the packaged `Cargo.lock` file if it is available. This may be useful for
ensuring reproducible builds, to use the exact same set of dependencies that
were available when the package was published. It may also be useful if a
newer version of a dependency is published that no longer builds on your
system, or has other problems. The downside to using `--locked` is that you
will not receive any fixes or updates to any dependency. Note that Cargo did
not start publishing `Cargo.lock` files until version 1.37, which means
packages published with prior versions will not have a `Cargo.lock` file
available.

### Configuration Discovery

This command operates on system or user level, not project level.
This means that the local [configuration discovery] is ignored.
Instead, the configuration discovery begins at `$CARGO_HOME/config.toml`. 
If the package is installed with `--path $PATH`, the local configuration 
will be used, beginning discovery at `$PATH/.cargo/config.toml`.

[configuration discovery]: ../reference/config.html#hierarchical-structure

## OPTIONS

### Install Options

<dl>

<dt class="option-term" id="option-cargo-install---vers"><a class="option-anchor" href="#option-cargo-install---vers"></a><code>--vers</code> <em>version</em></dt>
<dt class="option-term" id="option-cargo-install---version"><a class="option-anchor" href="#option-cargo-install---version"></a><code>--version</code> <em>version</em></dt>
<dd class="option-desc">Specify a version to install. This may be a <a href="../reference/specifying-dependencies.md">version
requirement</a>, like <code>~1.2</code>, to have Cargo
select the newest version from the given requirement. If the version does not
have a requirement operator (such as <code>^</code> or <code>~</code>), then it must be in the form
<em>MAJOR.MINOR.PATCH</em>, and will install exactly that version; it is <em>not</em>
treated as a caret requirement like Cargo dependencies are.</dd>


<dt class="option-term" id="option-cargo-install---git"><a class="option-anchor" href="#option-cargo-install---git"></a><code>--git</code> <em>url</em></dt>
<dd class="option-desc">Git URL to install the specified crate from.</dd>


<dt class="option-term" id="option-cargo-install---branch"><a class="option-anchor" href="#option-cargo-install---branch"></a><code>--branch</code> <em>branch</em></dt>
<dd class="option-desc">Branch to use when installing from git.</dd>


<dt class="option-term" id="option-cargo-install---tag"><a class="option-anchor" href="#option-cargo-install---tag"></a><code>--tag</code> <em>tag</em></dt>
<dd class="option-desc">Tag to use when installing from git.</dd>


<dt class="option-term" id="option-cargo-install---rev"><a class="option-anchor" href="#option-cargo-install---rev"></a><code>--rev</code> <em>sha</em></dt>
<dd class="option-desc">Specific commit to use when installing from git.</dd>


<dt class="option-term" id="option-cargo-install---path"><a class="option-anchor" href="#option-cargo-install---path"></a><code>--path</code> <em>path</em></dt>
<dd class="option-desc">Filesystem path to local crate to install.</dd>


<dt class="option-term" id="option-cargo-install---list"><a class="option-anchor" href="#option-cargo-install---list"></a><code>--list</code></dt>
<dd class="option-desc">List all installed packages and their versions.</dd>


<dt class="option-term" id="option-cargo-install--f"><a class="option-anchor" href="#option-cargo-install--f"></a><code>-f</code></dt>
<dt class="option-term" id="option-cargo-install---force"><a class="option-anchor" href="#option-cargo-install---force"></a><code>--force</code></dt>
<dd class="option-desc">Force overwriting existing crates or binaries. This can be used if a package
has installed a binary with the same name as another package. This is also
useful if something has changed on the system that you want to rebuild with,
such as a newer version of <code>rustc</code>.</dd>


<dt class="option-term" id="option-cargo-install---no-track"><a class="option-anchor" href="#option-cargo-install---no-track"></a><code>--no-track</code></dt>
<dd class="option-desc">By default, Cargo keeps track of the installed packages with a metadata file
stored in the installation root directory. This flag tells Cargo not to use or
create that file. With this flag, Cargo will refuse to overwrite any existing
files unless the <code>--force</code> flag is used. This also disables Cargo’s ability to
protect against multiple concurrent invocations of Cargo installing at the
same time.</dd>


<dt class="option-term" id="option-cargo-install---bin"><a class="option-anchor" href="#option-cargo-install---bin"></a><code>--bin</code> <em>name</em>…</dt>
<dd class="option-desc">Install only the specified binary.</dd>


<dt class="option-term" id="option-cargo-install---bins"><a class="option-anchor" href="#option-cargo-install---bins"></a><code>--bins</code></dt>
<dd class="option-desc">Install all binaries. This is the default behavior.</dd>


<dt class="option-term" id="option-cargo-install---example"><a class="option-anchor" href="#option-cargo-install---example"></a><code>--example</code> <em>name</em>…</dt>
<dd class="option-desc">Install only the specified example.</dd>


<dt class="option-term" id="option-cargo-install---examples"><a class="option-anchor" href="#option-cargo-install---examples"></a><code>--examples</code></dt>
<dd class="option-desc">Install all examples.</dd>


<dt class="option-term" id="option-cargo-install---root"><a class="option-anchor" href="#option-cargo-install---root"></a><code>--root</code> <em>dir</em></dt>
<dd class="option-desc">Directory to install packages into.</dd>


<dt class="option-term" id="option-cargo-install---registry"><a class="option-anchor" href="#option-cargo-install---registry"></a><code>--registry</code> <em>registry</em></dt>
<dd class="option-desc">Name of the registry to use. Registry names are defined in <a href="../reference/config.html">Cargo config
files</a>. If not specified, the default registry is used,
which is defined by the <code>registry.default</code> config key which defaults to
<code>crates-io</code>.</dd>



<dt class="option-term" id="option-cargo-install---index"><a class="option-anchor" href="#option-cargo-install---index"></a><code>--index</code> <em>index</em></dt>
<dd class="option-desc">The URL of the registry index to use.</dd>



</dl>

### Feature Selection

The feature flags allow you to control which features are enabled. When no
feature options are given, the `default` feature is activated for every
selected package.

See [the features documentation](../reference/features.html#command-line-feature-options)
for more details.

<dl>

<dt class="option-term" id="option-cargo-install--F"><a class="option-anchor" href="#option-cargo-install--F"></a><code>-F</code> <em>features</em></dt>
<dt class="option-term" id="option-cargo-install---features"><a class="option-anchor" href="#option-cargo-install---features"></a><code>--features</code> <em>features</em></dt>
<dd class="option-desc">Space or comma separated list of features to activate. Features of workspace
members may be enabled with <code>package-name/feature-name</code> syntax. This flag may
be specified multiple times, which enables all specified features.</dd>


<dt class="option-term" id="option-cargo-install---all-features"><a class="option-anchor" href="#option-cargo-install---all-features"></a><code>--all-features</code></dt>
<dd class="option-desc">Activate all available features of all selected packages.</dd>


<dt class="option-term" id="option-cargo-install---no-default-features"><a class="option-anchor" href="#option-cargo-install---no-default-features"></a><code>--no-default-features</code></dt>
<dd class="option-desc">Do not activate the <code>default</code> feature of the selected packages.</dd>


</dl>


### Compilation Options

<dl>

<dt class="option-term" id="option-cargo-install---target"><a class="option-anchor" href="#option-cargo-install---target"></a><code>--target</code> <em>triple</em></dt>
<dd class="option-desc">Install for the given architecture. The default is the host architecture. The general format of the triple is
<code>&lt;arch&gt;&lt;sub&gt;-&lt;vendor&gt;-&lt;sys&gt;-&lt;abi&gt;</code>. Run <code>rustc --print target-list</code> for a
list of supported targets.</p>
<p>This may also be specified with the <code>build.target</code>
<a href="../reference/config.html">config value</a>.</p>
<p>Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
<a href="../guide/build-cache.html">build cache</a> documentation for more details.</dd>



<dt class="option-term" id="option-cargo-install---target-dir"><a class="option-anchor" href="#option-cargo-install---target-dir"></a><code>--target-dir</code> <em>directory</em></dt>
<dd class="option-desc">Directory for all generated artifacts and intermediate files. May also be
specified with the <code>CARGO_TARGET_DIR</code> environment variable, or the
<code>build.target-dir</code> <a href="../reference/config.html">config value</a>.
Defaults to a new temporary folder located in the
temporary directory of the platform. </p>
<p>When using <code>--path</code>, by default it will use <code>target</code> directory in the workspace
of the local crate unless <code>--target-dir</code>
is specified.</dd>



<dt class="option-term" id="option-cargo-install---debug"><a class="option-anchor" href="#option-cargo-install---debug"></a><code>--debug</code></dt>
<dd class="option-desc">Build with the <code>dev</code> profile instead of the <code>release</code> profile.
See also the <code>--profile</code> option for choosing a specific profile by name.</dd>


<dt class="option-term" id="option-cargo-install---profile"><a class="option-anchor" href="#option-cargo-install---profile"></a><code>--profile</code> <em>name</em></dt>
<dd class="option-desc">Install with the given profile.
See <a href="../reference/profiles.html">the reference</a> for more details on profiles.</dd>



<dt class="option-term" id="option-cargo-install---ignore-rust-version"><a class="option-anchor" href="#option-cargo-install---ignore-rust-version"></a><code>--ignore-rust-version</code></dt>
<dd class="option-desc">Install the target even if the selected Rust compiler is older than the
required Rust version as configured in the project’s <code>rust-version</code> field.</dd>



<dt class="option-term" id="option-cargo-install---timings=fmts"><a class="option-anchor" href="#option-cargo-install---timings=fmts"></a><code>--timings=</code><em>fmts</em></dt>
<dd class="option-desc">Output information how long each compilation takes, and track concurrency
information over time. Accepts an optional comma-separated list of output
formats; <code>--timings</code> without an argument will default to <code>--timings=html</code>.
Specifying an output format (rather than the default) is unstable and requires
<code>-Zunstable-options</code>. Valid output formats:</p>
<ul>
<li><code>html</code> (unstable, requires <code>-Zunstable-options</code>): Write a human-readable file <code>cargo-timing.html</code> to the
<code>target/cargo-timings</code> directory with a report of the compilation. Also write
a report to the same directory with a timestamp in the filename if you want
to look at older runs. HTML output is suitable for human consumption only,
and does not provide machine-readable timing data.</li>
<li><code>json</code> (unstable, requires <code>-Zunstable-options</code>): Emit machine-readable JSON
information about timing information.</li>
</ul></dd>




</dl>

### Manifest Options

<dl>
<dt class="option-term" id="option-cargo-install---frozen"><a class="option-anchor" href="#option-cargo-install---frozen"></a><code>--frozen</code></dt>
<dt class="option-term" id="option-cargo-install---locked"><a class="option-anchor" href="#option-cargo-install---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Either of these flags requires that the <code>Cargo.lock</code> file is
up-to-date. If the lock file is missing, or it needs to be updated, Cargo will
exit with an error. The <code>--frozen</code> flag also prevents Cargo from
attempting to access the network to determine if it is out-of-date.</p>
<p>These may be used in environments where you want to assert that the
<code>Cargo.lock</code> file is up-to-date (such as a CI build) or want to avoid network
access.</dd>


<dt class="option-term" id="option-cargo-install---offline"><a class="option-anchor" href="#option-cargo-install---offline"></a><code>--offline</code></dt>
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


</dl>

### Miscellaneous Options

<dl>
<dt class="option-term" id="option-cargo-install--j"><a class="option-anchor" href="#option-cargo-install--j"></a><code>-j</code> <em>N</em></dt>
<dt class="option-term" id="option-cargo-install---jobs"><a class="option-anchor" href="#option-cargo-install---jobs"></a><code>--jobs</code> <em>N</em></dt>
<dd class="option-desc">Number of parallel jobs to run. May also be specified with the
<code>build.jobs</code> <a href="../reference/config.html">config value</a>. Defaults to
the number of logical CPUs. If negative, it sets the maximum number of
parallel jobs to the number of logical CPUs plus provided value. If
a string <code>default</code> is provided, it sets the value back to defaults.
Should not be 0.</dd>


<dt class="option-term" id="option-cargo-install---keep-going"><a class="option-anchor" href="#option-cargo-install---keep-going"></a><code>--keep-going</code></dt>
<dd class="option-desc">Build as many crates in the dependency graph as possible, rather than aborting
the build on the first one that fails to build.</p>
<p>For example if the current package depends on dependencies <code>fails</code> and <code>works</code>,
one of which fails to build, <code>cargo install -j1</code> may or may not build the
one that succeeds (depending on which one of the two builds Cargo picked to run
first), whereas <code>cargo install -j1 --keep-going</code> would definitely run both
builds, even if the one run first fails.</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-install--v"><a class="option-anchor" href="#option-cargo-install--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-install---verbose"><a class="option-anchor" href="#option-cargo-install---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-install--q"><a class="option-anchor" href="#option-cargo-install--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-install---quiet"><a class="option-anchor" href="#option-cargo-install---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-install---color"><a class="option-anchor" href="#option-cargo-install---color"></a><code>--color</code> <em>when</em></dt>
<dd class="option-desc">Control when colored output is used. Valid values:</p>
<ul>
<li><code>auto</code> (default): Automatically detect if color support is available on the
terminal.</li>
<li><code>always</code>: Always display colors.</li>
<li><code>never</code>: Never display colors.</li>
</ul>
<p>May also be specified with the <code>term.color</code>
<a href="../reference/config.html">config value</a>.</dd>



<dt class="option-term" id="option-cargo-install---message-format"><a class="option-anchor" href="#option-cargo-install---message-format"></a><code>--message-format</code> <em>fmt</em></dt>
<dd class="option-desc">The output format for diagnostic messages. Can be specified multiple times
and consists of comma-separated values. Valid values:</p>
<ul>
<li><code>human</code> (default): Display in a human-readable text format. Conflicts with
<code>short</code> and <code>json</code>.</li>
<li><code>short</code>: Emit shorter, human-readable text messages. Conflicts with <code>human</code>
and <code>json</code>.</li>
<li><code>json</code>: Emit JSON messages to stdout. See
<a href="../reference/external-tools.html#json-messages">the reference</a>
for more details. Conflicts with <code>human</code> and <code>short</code>.</li>
<li><code>json-diagnostic-short</code>: Ensure the <code>rendered</code> field of JSON messages contains
the “short” rendering from rustc. Cannot be used with <code>human</code> or <code>short</code>.</li>
<li><code>json-diagnostic-rendered-ansi</code>: Ensure the <code>rendered</code> field of JSON messages
contains embedded ANSI color codes for respecting rustc’s default color
scheme. Cannot be used with <code>human</code> or <code>short</code>.</li>
<li><code>json-render-diagnostics</code>: Instruct Cargo to not include rustc diagnostics
in JSON messages printed, but instead Cargo itself should render the
JSON diagnostics coming from rustc. Cargo’s own JSON diagnostics and others
coming from rustc are still emitted. Cannot be used with <code>human</code> or <code>short</code>.</li>
</ul></dd>



</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-install-+toolchain"><a class="option-anchor" href="#option-cargo-install-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-install---config"><a class="option-anchor" href="#option-cargo-install---config"></a><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></dt>
<dd class="option-desc">Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</dd>


<dt class="option-term" id="option-cargo-install--C"><a class="option-anchor" href="#option-cargo-install--C"></a><code>-C</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</dd>


<dt class="option-term" id="option-cargo-install--h"><a class="option-anchor" href="#option-cargo-install--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-install---help"><a class="option-anchor" href="#option-cargo-install---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-install--Z"><a class="option-anchor" href="#option-cargo-install--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>


## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.


## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.


## EXAMPLES

1. Install or upgrade a package from crates.io:

       cargo install ripgrep

2. Install or reinstall the package in the current directory:

       cargo install --path .

3. View the list of installed packages:

       cargo install --list

## SEE ALSO
[cargo(1)](cargo.html), [cargo-uninstall(1)](cargo-uninstall.html), [cargo-search(1)](cargo-search.html), [cargo-publish(1)](cargo-publish.html)
