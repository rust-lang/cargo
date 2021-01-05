# cargo-doc(1)


## NAME

cargo-doc - Build a package's documentation

## SYNOPSIS

`cargo doc` [_options_]

## DESCRIPTION

Build the documentation for the local package and all dependencies. The output
is placed in `target/doc` in rustdoc's usual format.

## OPTIONS

### Documentation Options

<dl>

<dt class="option-term" id="option-cargo-doc---open"><a class="option-anchor" href="#option-cargo-doc---open"></a><code>--open</code></dt>
<dd class="option-desc">Open the docs in a browser after building them. This will use your default
browser unless you define another one in the <code>BROWSER</code> environment variable.</dd>


<dt class="option-term" id="option-cargo-doc---no-deps"><a class="option-anchor" href="#option-cargo-doc---no-deps"></a><code>--no-deps</code></dt>
<dd class="option-desc">Do not build documentation for dependencies.</dd>


<dt class="option-term" id="option-cargo-doc---document-private-items"><a class="option-anchor" href="#option-cargo-doc---document-private-items"></a><code>--document-private-items</code></dt>
<dd class="option-desc">Include non-public items in the documentation.</dd>


</dl>

### Package Selection

By default, when no package selection options are given, the packages selected
depend on the selected manifest file (based on the current working directory if
`--manifest-path` is not given). If the manifest is the root of a workspace then
the workspaces default members are selected, otherwise only the package defined
by the manifest will be selected.

The default members of a workspace can be set explicitly with the
`workspace.default-members` key in the root manifest. If this is not set, a
virtual workspace will include all workspace members (equivalent to passing
`--workspace`), and a non-virtual workspace will include only the root crate itself.

<dl>

<dt class="option-term" id="option-cargo-doc--p"><a class="option-anchor" href="#option-cargo-doc--p"></a><code>-p</code> <em>spec</em>...</dt>
<dt class="option-term" id="option-cargo-doc---package"><a class="option-anchor" href="#option-cargo-doc---package"></a><code>--package</code> <em>spec</em>...</dt>
<dd class="option-desc">Document only the specified packages. See <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the
SPEC format. This flag may be specified multiple times and supports common Unix
glob patterns like <code>*</code>, <code>?</code> and <code>[]</code>. However, to avoid your shell accidentally 
expanding glob patterns before Cargo handles them, you must use single quotes or
double quotes around each pattern.</dd>


<dt class="option-term" id="option-cargo-doc---workspace"><a class="option-anchor" href="#option-cargo-doc---workspace"></a><code>--workspace</code></dt>
<dd class="option-desc">Document all members in the workspace.</dd>



<dt class="option-term" id="option-cargo-doc---all"><a class="option-anchor" href="#option-cargo-doc---all"></a><code>--all</code></dt>
<dd class="option-desc">Deprecated alias for <code>--workspace</code>.</dd>



<dt class="option-term" id="option-cargo-doc---exclude"><a class="option-anchor" href="#option-cargo-doc---exclude"></a><code>--exclude</code> <em>SPEC</em>...</dt>
<dd class="option-desc">Exclude the specified packages. Must be used in conjunction with the
<code>--workspace</code> flag. This flag may be specified multiple times and supports
common Unix glob patterns like <code>*</code>, <code>?</code> and <code>[]</code>. However, to avoid your shell
accidentally expanding glob patterns before Cargo handles them, you must use
single quotes or double quotes around each pattern.</dd>


</dl>


### Target Selection

When no target selection options are given, `cargo doc` will document all
binary and library targets of the selected package. The binary will be skipped
if its name is the same as the lib target. Binaries are skipped if they have
`required-features` that are missing.

The default behavior can be changed by setting `doc = false` for the target in
the manifest settings. Using target selection options will ignore the `doc`
flag and will always document the given target.

<dl>
<dt class="option-term" id="option-cargo-doc---lib"><a class="option-anchor" href="#option-cargo-doc---lib"></a><code>--lib</code></dt>
<dd class="option-desc">Document the package's library.</dd>


<dt class="option-term" id="option-cargo-doc---bin"><a class="option-anchor" href="#option-cargo-doc---bin"></a><code>--bin</code> <em>name</em>...</dt>
<dd class="option-desc">Document the specified binary. This flag may be specified multiple times
and supports common Unix glob patterns.</dd>


<dt class="option-term" id="option-cargo-doc---bins"><a class="option-anchor" href="#option-cargo-doc---bins"></a><code>--bins</code></dt>
<dd class="option-desc">Document all binary targets.</dd>


</dl>

### Feature Selection

The feature flags allow you to control which features are enabled. When no
feature options are given, the `default` feature is activated for every
selected package.

See [the features documentation](../reference/features.html#command-line-feature-options)
for more details.

<dl>

<dt class="option-term" id="option-cargo-doc---features"><a class="option-anchor" href="#option-cargo-doc---features"></a><code>--features</code> <em>features</em></dt>
<dd class="option-desc">Space or comma separated list of features to activate. Features of workspace
members may be enabled with <code>package-name/feature-name</code> syntax. This flag may
be specified multiple times, which enables all specified features.</dd>


<dt class="option-term" id="option-cargo-doc---all-features"><a class="option-anchor" href="#option-cargo-doc---all-features"></a><code>--all-features</code></dt>
<dd class="option-desc">Activate all available features of all selected packages.</dd>


<dt class="option-term" id="option-cargo-doc---no-default-features"><a class="option-anchor" href="#option-cargo-doc---no-default-features"></a><code>--no-default-features</code></dt>
<dd class="option-desc">Do not activate the <code>default</code> feature of the selected packages.</dd>


</dl>


### Compilation Options

<dl>

<dt class="option-term" id="option-cargo-doc---target"><a class="option-anchor" href="#option-cargo-doc---target"></a><code>--target</code> <em>triple</em></dt>
<dd class="option-desc">Document for the given architecture. The default is the host
architecture. The general format of the triple is
<code>&lt;arch&gt;&lt;sub&gt;-&lt;vendor&gt;-&lt;sys&gt;-&lt;abi&gt;</code>. Run <code>rustc --print target-list</code> for a
list of supported targets.</p>
<p>This may also be specified with the <code>build.target</code>
<a href="../reference/config.html">config value</a>.</p>
<p>Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
<a href="../guide/build-cache.html">build cache</a> documentation for more details.</dd>



<dt class="option-term" id="option-cargo-doc---release"><a class="option-anchor" href="#option-cargo-doc---release"></a><code>--release</code></dt>
<dd class="option-desc">Document optimized artifacts with the <code>release</code> profile. See the
<a href="#profiles">PROFILES</a> section for details on how this affects profile
selection.</dd>



</dl>

### Output Options

<dl>
<dt class="option-term" id="option-cargo-doc---target-dir"><a class="option-anchor" href="#option-cargo-doc---target-dir"></a><code>--target-dir</code> <em>directory</em></dt>
<dd class="option-desc">Directory for all generated artifacts and intermediate files. May also be
specified with the <code>CARGO_TARGET_DIR</code> environment variable, or the
<code>build.target-dir</code> <a href="../reference/config.html">config value</a>. Defaults
to <code>target</code> in the root of the workspace.</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-doc--v"><a class="option-anchor" href="#option-cargo-doc--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-doc---verbose"><a class="option-anchor" href="#option-cargo-doc---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for &quot;very verbose&quot; output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-doc--q"><a class="option-anchor" href="#option-cargo-doc--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-doc---quiet"><a class="option-anchor" href="#option-cargo-doc---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">No output printed to stdout.</dd>


<dt class="option-term" id="option-cargo-doc---color"><a class="option-anchor" href="#option-cargo-doc---color"></a><code>--color</code> <em>when</em></dt>
<dd class="option-desc">Control when colored output is used. Valid values:</p>
<ul>
<li><code>auto</code> (default): Automatically detect if color support is available on the
terminal.</li>
<li><code>always</code>: Always display colors.</li>
<li><code>never</code>: Never display colors.</li>
</ul>
<p>May also be specified with the <code>term.color</code>
<a href="../reference/config.html">config value</a>.</dd>



<dt class="option-term" id="option-cargo-doc---message-format"><a class="option-anchor" href="#option-cargo-doc---message-format"></a><code>--message-format</code> <em>fmt</em></dt>
<dd class="option-desc">The output format for diagnostic messages. Can be specified multiple times
and consists of comma-separated values. Valid values:</p>
<ul>
<li><code>human</code> (default): Display in a human-readable text format.</li>
<li><code>short</code>: Emit shorter, human-readable text messages.</li>
<li><code>json</code>: Emit JSON messages to stdout. See
<a href="../reference/external-tools.html#json-messages">the reference</a>
for more details.</li>
<li><code>json-diagnostic-short</code>: Ensure the <code>rendered</code> field of JSON messages contains
the &quot;short&quot; rendering from rustc.</li>
<li><code>json-diagnostic-rendered-ansi</code>: Ensure the <code>rendered</code> field of JSON messages
contains embedded ANSI color codes for respecting rustc's default color
scheme.</li>
<li><code>json-render-diagnostics</code>: Instruct Cargo to not include rustc diagnostics in
in JSON messages printed, but instead Cargo itself should render the
JSON diagnostics coming from rustc. Cargo's own JSON diagnostics and others
coming from rustc are still emitted.</li>
</ul></dd>


</dl>

### Manifest Options

<dl>
<dt class="option-term" id="option-cargo-doc---manifest-path"><a class="option-anchor" href="#option-cargo-doc---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>



<dt class="option-term" id="option-cargo-doc---frozen"><a class="option-anchor" href="#option-cargo-doc---frozen"></a><code>--frozen</code></dt>
<dt class="option-term" id="option-cargo-doc---locked"><a class="option-anchor" href="#option-cargo-doc---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Either of these flags requires that the <code>Cargo.lock</code> file is
up-to-date. If the lock file is missing, or it needs to be updated, Cargo will
exit with an error. The <code>--frozen</code> flag also prevents Cargo from
attempting to access the network to determine if it is out-of-date.</p>
<p>These may be used in environments where you want to assert that the
<code>Cargo.lock</code> file is up-to-date (such as a CI build) or want to avoid network
access.</dd>


<dt class="option-term" id="option-cargo-doc---offline"><a class="option-anchor" href="#option-cargo-doc---offline"></a><code>--offline</code></dt>
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

### Common Options

<dl>

<dt class="option-term" id="option-cargo-doc-+toolchain"><a class="option-anchor" href="#option-cargo-doc-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-doc--h"><a class="option-anchor" href="#option-cargo-doc--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-doc---help"><a class="option-anchor" href="#option-cargo-doc---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-doc--Z"><a class="option-anchor" href="#option-cargo-doc--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>


### Miscellaneous Options

<dl>
<dt class="option-term" id="option-cargo-doc--j"><a class="option-anchor" href="#option-cargo-doc--j"></a><code>-j</code> <em>N</em></dt>
<dt class="option-term" id="option-cargo-doc---jobs"><a class="option-anchor" href="#option-cargo-doc---jobs"></a><code>--jobs</code> <em>N</em></dt>
<dd class="option-desc">Number of parallel jobs to run. May also be specified with the
<code>build.jobs</code> <a href="../reference/config.html">config value</a>. Defaults to
the number of CPUs.</dd>


</dl>

## PROFILES

Profiles may be used to configure compiler options such as optimization levels
and debug settings. See [the reference](../reference/profiles.html) for more
details.

Profile selection depends on the target and crate being built. By default the
`dev` or `test` profiles are used. If the `--release` flag is given, then the
`release` or `bench` profiles are used.

Target | Default Profile | `--release` Profile
-------|-----------------|---------------------
lib, bin, example | `dev` | `release`
test, bench, or any target in "test" or "bench" mode | `test` | `bench`

Dependencies use the `dev`/`release` profiles.


## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.


## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.


## EXAMPLES

1. Build the local package documentation and its dependencies and output to
   `target/doc`.

       cargo doc

## SEE ALSO
[cargo(1)](cargo.html), [cargo-rustdoc(1)](cargo-rustdoc.html), [rustdoc(1)](https://doc.rust-lang.org/rustdoc/index.html)
