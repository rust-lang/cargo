# cargo-fix(1)


## NAME

cargo-fix - Automatically fix lint warnings reported by rustc

## SYNOPSIS

`cargo fix` [_options_]

## DESCRIPTION

This Cargo subcommand will automatically take rustc's suggestions from
diagnostics like warnings and apply them to your source code. This is intended
to help automate tasks that rustc itself already knows how to tell you to fix!
The `cargo fix` subcommand is also being developed for the Rust 2018 edition
to provide code the ability to easily opt-in to the new edition without having
to worry about any breakage.

Executing `cargo fix` will under the hood execute [cargo-check(1)](cargo-check.md). Any warnings
applicable to your crate will be automatically fixed (if possible) and all
remaining warnings will be displayed when the check process is finished. For
example if you'd like to prepare for the 2018 edition, you can do so by
executing:

    cargo fix --edition

which behaves the same as `cargo check --all-targets`.

`cargo fix` is only capable of fixing code that is normally compiled with
`cargo check`. If code is conditionally enabled with optional features, you
will need to enable those features for that code to be analyzed:

    cargo fix --edition --features foo

Similarly, other `cfg` expressions like platform-specific code will need to
pass `--target` to fix code for the given target.

    cargo fix --edition --target x86_64-pc-windows-gnu

If you encounter any problems with `cargo fix` or otherwise have any questions
or feature requests please don't hesitate to file an issue at
<https://github.com/rust-lang/cargo>

## OPTIONS

### Fix options

<dl>

<dt class="option-term" id="option-cargo-fix---broken-code"><a class="option-anchor" href="#option-cargo-fix---broken-code"></a><code>--broken-code</code></dt>
<dd class="option-desc">Fix code even if it already has compiler errors. This is useful if <code>cargo fix</code>
fails to apply the changes. It will apply the changes and leave the broken
code in the working directory for you to inspect and manually fix.</dd>


<dt class="option-term" id="option-cargo-fix---edition"><a class="option-anchor" href="#option-cargo-fix---edition"></a><code>--edition</code></dt>
<dd class="option-desc">Apply changes that will update the code to the latest edition. This will not
update the edition in the <code>Cargo.toml</code> manifest, which must be updated
manually.</dd>


<dt class="option-term" id="option-cargo-fix---edition-idioms"><a class="option-anchor" href="#option-cargo-fix---edition-idioms"></a><code>--edition-idioms</code></dt>
<dd class="option-desc">Apply suggestions that will update code to the preferred style for the current
edition.</dd>


<dt class="option-term" id="option-cargo-fix---allow-no-vcs"><a class="option-anchor" href="#option-cargo-fix---allow-no-vcs"></a><code>--allow-no-vcs</code></dt>
<dd class="option-desc">Fix code even if a VCS was not detected.</dd>


<dt class="option-term" id="option-cargo-fix---allow-dirty"><a class="option-anchor" href="#option-cargo-fix---allow-dirty"></a><code>--allow-dirty</code></dt>
<dd class="option-desc">Fix code even if the working directory has changes.</dd>


<dt class="option-term" id="option-cargo-fix---allow-staged"><a class="option-anchor" href="#option-cargo-fix---allow-staged"></a><code>--allow-staged</code></dt>
<dd class="option-desc">Fix code even if the working directory has staged changes.</dd>


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

<dt class="option-term" id="option-cargo-fix--p"><a class="option-anchor" href="#option-cargo-fix--p"></a><code>-p</code> <em>spec</em>...</dt>
<dt class="option-term" id="option-cargo-fix---package"><a class="option-anchor" href="#option-cargo-fix---package"></a><code>--package</code> <em>spec</em>...</dt>
<dd class="option-desc">Fix only the specified packages. See <a href="https://doc.rust-lang.org/cargo/commands/cargo-pkgid.md">cargo-pkgid(1)</a> for the
SPEC format. This flag may be specified multiple times.</dd>


<dt class="option-term" id="option-cargo-fix---workspace"><a class="option-anchor" href="#option-cargo-fix---workspace"></a><code>--workspace</code></dt>
<dd class="option-desc">Fix all members in the workspace.</dd>



<dt class="option-term" id="option-cargo-fix---all"><a class="option-anchor" href="#option-cargo-fix---all"></a><code>--all</code></dt>
<dd class="option-desc">Deprecated alias for <code>--workspace</code>.</dd>



<dt class="option-term" id="option-cargo-fix---exclude"><a class="option-anchor" href="#option-cargo-fix---exclude"></a><code>--exclude</code> <em>SPEC</em>...</dt>
<dd class="option-desc">Exclude the specified packages. Must be used in conjunction with the
<code>--workspace</code> flag. This flag may be specified multiple times.</dd>


</dl>


### Target Selection

When no target selection options are given, `cargo fix` will fix all targets
(`--all-targets` implied). Binaries are skipped if they have
`required-features` that are missing.

Passing target selection flags will fix only the specified
targets.

<dl>

<dt class="option-term" id="option-cargo-fix---lib"><a class="option-anchor" href="#option-cargo-fix---lib"></a><code>--lib</code></dt>
<dd class="option-desc">Fix the package's library.</dd>


<dt class="option-term" id="option-cargo-fix---bin"><a class="option-anchor" href="#option-cargo-fix---bin"></a><code>--bin</code> <em>name</em>...</dt>
<dd class="option-desc">Fix the specified binary. This flag may be specified multiple times.</dd>


<dt class="option-term" id="option-cargo-fix---bins"><a class="option-anchor" href="#option-cargo-fix---bins"></a><code>--bins</code></dt>
<dd class="option-desc">Fix all binary targets.</dd>



<dt class="option-term" id="option-cargo-fix---example"><a class="option-anchor" href="#option-cargo-fix---example"></a><code>--example</code> <em>name</em>...</dt>
<dd class="option-desc">Fix the specified example. This flag may be specified multiple times.</dd>


<dt class="option-term" id="option-cargo-fix---examples"><a class="option-anchor" href="#option-cargo-fix---examples"></a><code>--examples</code></dt>
<dd class="option-desc">Fix all example targets.</dd>


<dt class="option-term" id="option-cargo-fix---test"><a class="option-anchor" href="#option-cargo-fix---test"></a><code>--test</code> <em>name</em>...</dt>
<dd class="option-desc">Fix the specified integration test. This flag may be specified
multiple times.</dd>


<dt class="option-term" id="option-cargo-fix---tests"><a class="option-anchor" href="#option-cargo-fix---tests"></a><code>--tests</code></dt>
<dd class="option-desc">Fix all targets in test mode that have the <code>test = true</code> manifest
flag set. By default this includes the library and binaries built as
unittests, and integration tests. Be aware that this will also build any
required dependencies, so the lib target may be built twice (once as a
unittest, and once as a dependency for binaries, integration tests, etc.).
Targets may be enabled or disabled by setting the <code>test</code> flag in the
manifest settings for the target.</dd>


<dt class="option-term" id="option-cargo-fix---bench"><a class="option-anchor" href="#option-cargo-fix---bench"></a><code>--bench</code> <em>name</em>...</dt>
<dd class="option-desc">Fix the specified benchmark. This flag may be specified multiple times.</dd>


<dt class="option-term" id="option-cargo-fix---benches"><a class="option-anchor" href="#option-cargo-fix---benches"></a><code>--benches</code></dt>
<dd class="option-desc">Fix all targets in benchmark mode that have the <code>bench = true</code>
manifest flag set. By default this includes the library and binaries built
as benchmarks, and bench targets. Be aware that this will also build any
required dependencies, so the lib target may be built twice (once as a
benchmark, and once as a dependency for binaries, benchmarks, etc.).
Targets may be enabled or disabled by setting the <code>bench</code> flag in the
manifest settings for the target.</dd>


<dt class="option-term" id="option-cargo-fix---all-targets"><a class="option-anchor" href="#option-cargo-fix---all-targets"></a><code>--all-targets</code></dt>
<dd class="option-desc">Fix all targets. This is equivalent to specifying <code>--lib --bins --tests --benches --examples</code>.</dd>


</dl>


### Feature Selection

The feature flags allow you to control the enabled features for the "current"
package. The "current" package is the package in the current directory, or the
one specified in `--manifest-path`. If running in the root of a virtual
workspace, then the default features are selected for all workspace members,
or all features if `--all-features` is specified.

When no feature options are given, the `default` feature is activated for
every selected package.

<dl>

<dt class="option-term" id="option-cargo-fix---features"><a class="option-anchor" href="#option-cargo-fix---features"></a><code>--features</code> <em>features</em></dt>
<dd class="option-desc">Space or comma separated list of features to activate. These features only
apply to the current directory's package. Features of direct dependencies
may be enabled with <code>&lt;dep-name&gt;/&lt;feature-name&gt;</code> syntax. This flag may be
specified multiple times, which enables all specified features.</dd>


<dt class="option-term" id="option-cargo-fix---all-features"><a class="option-anchor" href="#option-cargo-fix---all-features"></a><code>--all-features</code></dt>
<dd class="option-desc">Activate all available features of all selected packages.</dd>


<dt class="option-term" id="option-cargo-fix---no-default-features"><a class="option-anchor" href="#option-cargo-fix---no-default-features"></a><code>--no-default-features</code></dt>
<dd class="option-desc">Do not activate the <code>default</code> feature of the current directory's package.</dd>


</dl>


### Compilation Options

<dl>

<dt class="option-term" id="option-cargo-fix---target"><a class="option-anchor" href="#option-cargo-fix---target"></a><code>--target</code> <em>triple</em></dt>
<dd class="option-desc">Fix for the given architecture. The default is the host
architecture. The general format of the triple is
<code>&lt;arch&gt;&lt;sub&gt;-&lt;vendor&gt;-&lt;sys&gt;-&lt;abi&gt;</code>. Run <code>rustc --print target-list</code> for a
list of supported targets.</p>
<p>This may also be specified with the <code>build.target</code>
<a href="https://doc.rust-lang.org/cargo/reference/config.html">config value</a>.</p>
<p>Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
<a href="https://doc.rust-lang.org/cargo/guide/build-cache.html">build cache</a> documentation for more details.</dd>



<dt class="option-term" id="option-cargo-fix---release"><a class="option-anchor" href="#option-cargo-fix---release"></a><code>--release</code></dt>
<dd class="option-desc">Fix optimized artifacts with the <code>release</code> profile. See the
<a href="#profiles">PROFILES</a> section for details on how this affects profile
selection.</dd>



<dt class="option-term" id="option-cargo-fix---profile"><a class="option-anchor" href="#option-cargo-fix---profile"></a><code>--profile</code> <em>name</em></dt>
<dd class="option-desc">Changes fix behavior. Currently only <code>test</code> is supported,
which will fix with the <code>#[cfg(test)]</code> attribute enabled.
This is useful to have it fix unit tests which are usually
excluded via the <code>cfg</code> attribute. This does not change the actual profile
used.</dd>



</dl>

### Output Options

<dl>
<dt class="option-term" id="option-cargo-fix---target-dir"><a class="option-anchor" href="#option-cargo-fix---target-dir"></a><code>--target-dir</code> <em>directory</em></dt>
<dd class="option-desc">Directory for all generated artifacts and intermediate files. May also be
specified with the <code>CARGO_TARGET_DIR</code> environment variable, or the
<code>build.target-dir</code> <a href="https://doc.rust-lang.org/cargo/reference/config.html">config value</a>. Defaults
to <code>target</code> in the root of the workspace.</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-fix--v"><a class="option-anchor" href="#option-cargo-fix--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-fix---verbose"><a class="option-anchor" href="#option-cargo-fix---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for &quot;very verbose&quot; output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="https://doc.rust-lang.org/cargo/reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-fix--q"><a class="option-anchor" href="#option-cargo-fix--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-fix---quiet"><a class="option-anchor" href="#option-cargo-fix---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">No output printed to stdout.</dd>


<dt class="option-term" id="option-cargo-fix---color"><a class="option-anchor" href="#option-cargo-fix---color"></a><code>--color</code> <em>when</em></dt>
<dd class="option-desc">Control when colored output is used. Valid values:</p>
<ul>
<li><code>auto</code> (default): Automatically detect if color support is available on the
terminal.</li>
<li><code>always</code>: Always display colors.</li>
<li><code>never</code>: Never display colors.</li>
</ul>
<p>May also be specified with the <code>term.color</code>
<a href="https://doc.rust-lang.org/cargo/reference/config.html">config value</a>.</dd>



<dt class="option-term" id="option-cargo-fix---message-format"><a class="option-anchor" href="#option-cargo-fix---message-format"></a><code>--message-format</code> <em>fmt</em></dt>
<dd class="option-desc">The output format for diagnostic messages. Can be specified multiple times
and consists of comma-separated values. Valid values:</p>
<ul>
<li><code>human</code> (default): Display in a human-readable text format.</li>
<li><code>short</code>: Emit shorter, human-readable text messages.</li>
<li><code>json</code>: Emit JSON messages to stdout. See
<a href="https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages">the reference</a>
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
<dt class="option-term" id="option-cargo-fix---manifest-path"><a class="option-anchor" href="#option-cargo-fix---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>



<dt class="option-term" id="option-cargo-fix---frozen"><a class="option-anchor" href="#option-cargo-fix---frozen"></a><code>--frozen</code></dt>
<dt class="option-term" id="option-cargo-fix---locked"><a class="option-anchor" href="#option-cargo-fix---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Either of these flags requires that the <code>Cargo.lock</code> file is
up-to-date. If the lock file is missing, or it needs to be updated, Cargo will
exit with an error. The <code>--frozen</code> flag also prevents Cargo from
attempting to access the network to determine if it is out-of-date.</p>
<p>These may be used in environments where you want to assert that the
<code>Cargo.lock</code> file is up-to-date (such as a CI build) or want to avoid network
access.</dd>


<dt class="option-term" id="option-cargo-fix---offline"><a class="option-anchor" href="#option-cargo-fix---offline"></a><code>--offline</code></dt>
<dd class="option-desc">Prevents Cargo from accessing the network for any reason. Without this
flag, Cargo will stop with an error if it needs to access the network and
the network is not available. With this flag, Cargo will attempt to
proceed without the network if possible.</p>
<p>Beware that this may result in different dependency resolution than online
mode. Cargo will restrict itself to crates that are downloaded locally, even
if there might be a newer version as indicated in the local copy of the index.
See the <a href="https://doc.rust-lang.org/cargo/commands/cargo-fetch.md">cargo-fetch(1)</a> command to download dependencies before going
offline.</p>
<p>May also be specified with the <code>net.offline</code> <a href="https://doc.rust-lang.org/cargo/reference/config.html">config value</a>.</dd>


</dl>

### Common Options

<dl>

<dt class="option-term" id="option-cargo-fix-+toolchain"><a class="option-anchor" href="#option-cargo-fix-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://github.com/rust-lang/rustup/">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-fix--h"><a class="option-anchor" href="#option-cargo-fix--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-fix---help"><a class="option-anchor" href="#option-cargo-fix---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-fix--Z"><a class="option-anchor" href="#option-cargo-fix--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>


### Miscellaneous Options

<dl>
<dt class="option-term" id="option-cargo-fix--j"><a class="option-anchor" href="#option-cargo-fix--j"></a><code>-j</code> <em>N</em></dt>
<dt class="option-term" id="option-cargo-fix---jobs"><a class="option-anchor" href="#option-cargo-fix---jobs"></a><code>--jobs</code> <em>N</em></dt>
<dd class="option-desc">Number of parallel jobs to run. May also be specified with the
<code>build.jobs</code> <a href="https://doc.rust-lang.org/cargo/reference/config.html">config value</a>. Defaults to
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

1. Apply compiler suggestions to the local package:

       cargo fix

2. Convert a 2015 edition to 2018:

       cargo fix --edition

3. Apply suggested idioms for the current edition:

       cargo fix --edition-idioms

## SEE ALSO
[cargo(1)](cargo.md), [cargo-check(1)](cargo-check.md)
