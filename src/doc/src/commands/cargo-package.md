# cargo-package(1)

## NAME

cargo-package --- Assemble the local package into a distributable tarball

## SYNOPSIS

`cargo package` [_options_]

## DESCRIPTION

This command will create a distributable, compressed `.crate` file with the
source code of the package in the current directory. The resulting file will
be stored in the `target/package` directory. This performs the following
steps:

1. Load and check the current workspace, performing some basic checks.
    - Path dependencies are not allowed unless they have a version key. Cargo
      will ignore the path key for dependencies in published packages.
      `dev-dependencies` do not have this restriction.
2. Create the compressed `.crate` file.
    - The original `Cargo.toml` file is rewritten and normalized.
    - `[patch]`, `[replace]`, and `[workspace]` sections are removed from the
      manifest.
    - `Cargo.lock` is automatically included if the package contains an
      executable binary or example target. [cargo-install(1)](cargo-install.html) will use the
      packaged lock file if the `--locked` flag is used.
    - A `.cargo_vcs_info.json` file is included that contains information
      about the current VCS checkout hash if available (not included with
      `--allow-dirty`).
3. Extract the `.crate` file and build it to verify it can build.
    - This will rebuild your package from scratch to ensure that it can be
      built from a pristine state. The `--no-verify` flag can be used to skip
      this step.
4. Check that build scripts did not modify any source files.

The list of files included can be controlled with the `include` and `exclude`
fields in the manifest.

See [the reference](../reference/publishing.html) for more details about
packaging and publishing.

### .cargo_vcs_info.json format

Will generate a `.cargo_vcs_info.json` in the following format

```javascript
{
 "git": {
   "sha1": "aac20b6e7e543e6dd4118b246c77225e3a3a1302"
 },
 "path_in_vcs": ""
}
```

`path_in_vcs` will be set to a repo-relative path for packages
in subdirectories of the version control repository.

## OPTIONS

### Package Options

<dl>

<dt class="option-term" id="option-cargo-package--l"><a class="option-anchor" href="#option-cargo-package--l"></a><code>-l</code></dt>
<dt class="option-term" id="option-cargo-package---list"><a class="option-anchor" href="#option-cargo-package---list"></a><code>--list</code></dt>
<dd class="option-desc">Print files included in a package without making one.</dd>


<dt class="option-term" id="option-cargo-package---no-verify"><a class="option-anchor" href="#option-cargo-package---no-verify"></a><code>--no-verify</code></dt>
<dd class="option-desc">Don’t verify the contents by building them.</dd>


<dt class="option-term" id="option-cargo-package---no-metadata"><a class="option-anchor" href="#option-cargo-package---no-metadata"></a><code>--no-metadata</code></dt>
<dd class="option-desc">Ignore warnings about a lack of human-usable metadata (such as the description
or the license).</dd>


<dt class="option-term" id="option-cargo-package---allow-dirty"><a class="option-anchor" href="#option-cargo-package---allow-dirty"></a><code>--allow-dirty</code></dt>
<dd class="option-desc">Allow working directories with uncommitted VCS changes to be packaged.</dd>


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

<dt class="option-term" id="option-cargo-package--p"><a class="option-anchor" href="#option-cargo-package--p"></a><code>-p</code> <em>spec</em>…</dt>
<dt class="option-term" id="option-cargo-package---package"><a class="option-anchor" href="#option-cargo-package---package"></a><code>--package</code> <em>spec</em>…</dt>
<dd class="option-desc">Package only the specified packages. See <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the
SPEC format. This flag may be specified multiple times and supports common Unix
glob patterns like <code>*</code>, <code>?</code> and <code>[]</code>. However, to avoid your shell accidentally 
expanding glob patterns before Cargo handles them, you must use single quotes or
double quotes around each pattern.</dd>


<dt class="option-term" id="option-cargo-package---workspace"><a class="option-anchor" href="#option-cargo-package---workspace"></a><code>--workspace</code></dt>
<dd class="option-desc">Package all members in the workspace.</dd>




<dt class="option-term" id="option-cargo-package---exclude"><a class="option-anchor" href="#option-cargo-package---exclude"></a><code>--exclude</code> <em>SPEC</em>…</dt>
<dd class="option-desc">Exclude the specified packages. Must be used in conjunction with the
<code>--workspace</code> flag. This flag may be specified multiple times and supports
common Unix glob patterns like <code>*</code>, <code>?</code> and <code>[]</code>. However, to avoid your shell
accidentally expanding glob patterns before Cargo handles them, you must use
single quotes or double quotes around each pattern.</dd>


</dl>


### Compilation Options

<dl>

<dt class="option-term" id="option-cargo-package---target"><a class="option-anchor" href="#option-cargo-package---target"></a><code>--target</code> <em>triple</em></dt>
<dd class="option-desc">Package for the given architecture. The default is the host architecture. The general format of the triple is
<code>&lt;arch&gt;&lt;sub&gt;-&lt;vendor&gt;-&lt;sys&gt;-&lt;abi&gt;</code>. Run <code>rustc --print target-list</code> for a
list of supported targets. This flag may be specified multiple times.</p>
<p>This may also be specified with the <code>build.target</code>
<a href="../reference/config.html">config value</a>.</p>
<p>Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
<a href="../guide/build-cache.html">build cache</a> documentation for more details.</dd>



<dt class="option-term" id="option-cargo-package---target-dir"><a class="option-anchor" href="#option-cargo-package---target-dir"></a><code>--target-dir</code> <em>directory</em></dt>
<dd class="option-desc">Directory for all generated artifacts and intermediate files. May also be
specified with the <code>CARGO_TARGET_DIR</code> environment variable, or the
<code>build.target-dir</code> <a href="../reference/config.html">config value</a>.
Defaults to <code>target</code> in the root of the workspace.</dd>



</dl>

### Feature Selection

The feature flags allow you to control which features are enabled. When no
feature options are given, the `default` feature is activated for every
selected package.

See [the features documentation](../reference/features.html#command-line-feature-options)
for more details.

<dl>

<dt class="option-term" id="option-cargo-package--F"><a class="option-anchor" href="#option-cargo-package--F"></a><code>-F</code> <em>features</em></dt>
<dt class="option-term" id="option-cargo-package---features"><a class="option-anchor" href="#option-cargo-package---features"></a><code>--features</code> <em>features</em></dt>
<dd class="option-desc">Space or comma separated list of features to activate. Features of workspace
members may be enabled with <code>package-name/feature-name</code> syntax. This flag may
be specified multiple times, which enables all specified features.</dd>


<dt class="option-term" id="option-cargo-package---all-features"><a class="option-anchor" href="#option-cargo-package---all-features"></a><code>--all-features</code></dt>
<dd class="option-desc">Activate all available features of all selected packages.</dd>


<dt class="option-term" id="option-cargo-package---no-default-features"><a class="option-anchor" href="#option-cargo-package---no-default-features"></a><code>--no-default-features</code></dt>
<dd class="option-desc">Do not activate the <code>default</code> feature of the selected packages.</dd>


</dl>


### Manifest Options

<dl>

<dt class="option-term" id="option-cargo-package---manifest-path"><a class="option-anchor" href="#option-cargo-package---manifest-path"></a><code>--manifest-path</code> <em>path</em></dt>
<dd class="option-desc">Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</dd>



<dt class="option-term" id="option-cargo-package---frozen"><a class="option-anchor" href="#option-cargo-package---frozen"></a><code>--frozen</code></dt>
<dt class="option-term" id="option-cargo-package---locked"><a class="option-anchor" href="#option-cargo-package---locked"></a><code>--locked</code></dt>
<dd class="option-desc">Either of these flags requires that the <code>Cargo.lock</code> file is
up-to-date. If the lock file is missing, or it needs to be updated, Cargo will
exit with an error. The <code>--frozen</code> flag also prevents Cargo from
attempting to access the network to determine if it is out-of-date.</p>
<p>These may be used in environments where you want to assert that the
<code>Cargo.lock</code> file is up-to-date (such as a CI build) or want to avoid network
access.</dd>


<dt class="option-term" id="option-cargo-package---offline"><a class="option-anchor" href="#option-cargo-package---offline"></a><code>--offline</code></dt>
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
<dt class="option-term" id="option-cargo-package--j"><a class="option-anchor" href="#option-cargo-package--j"></a><code>-j</code> <em>N</em></dt>
<dt class="option-term" id="option-cargo-package---jobs"><a class="option-anchor" href="#option-cargo-package---jobs"></a><code>--jobs</code> <em>N</em></dt>
<dd class="option-desc">Number of parallel jobs to run. May also be specified with the
<code>build.jobs</code> <a href="../reference/config.html">config value</a>. Defaults to
the number of logical CPUs. If negative, it sets the maximum number of
parallel jobs to the number of logical CPUs plus provided value. If
a string <code>default</code> is provided, it sets the value back to defaults.
Should not be 0.</dd>


<dt class="option-term" id="option-cargo-package---keep-going"><a class="option-anchor" href="#option-cargo-package---keep-going"></a><code>--keep-going</code></dt>
<dd class="option-desc">Build as many crates in the dependency graph as possible, rather than aborting
the build on the first one that fails to build.</p>
<p>For example if the current package depends on dependencies <code>fails</code> and <code>works</code>,
one of which fails to build, <code>cargo package -j1</code> may or may not build the
one that succeeds (depending on which one of the two builds Cargo picked to run
first), whereas <code>cargo package -j1 --keep-going</code> would definitely run both
builds, even if the one run first fails.</dd>


</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-package--v"><a class="option-anchor" href="#option-cargo-package--v"></a><code>-v</code></dt>
<dt class="option-term" id="option-cargo-package---verbose"><a class="option-anchor" href="#option-cargo-package---verbose"></a><code>--verbose</code></dt>
<dd class="option-desc">Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-package--q"><a class="option-anchor" href="#option-cargo-package--q"></a><code>-q</code></dt>
<dt class="option-term" id="option-cargo-package---quiet"><a class="option-anchor" href="#option-cargo-package---quiet"></a><code>--quiet</code></dt>
<dd class="option-desc">Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</dd>


<dt class="option-term" id="option-cargo-package---color"><a class="option-anchor" href="#option-cargo-package---color"></a><code>--color</code> <em>when</em></dt>
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

<dt class="option-term" id="option-cargo-package-+toolchain"><a class="option-anchor" href="#option-cargo-package-+toolchain"></a><code>+</code><em>toolchain</em></dt>
<dd class="option-desc">If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</dd>


<dt class="option-term" id="option-cargo-package---config"><a class="option-anchor" href="#option-cargo-package---config"></a><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></dt>
<dd class="option-desc">Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</dd>


<dt class="option-term" id="option-cargo-package--C"><a class="option-anchor" href="#option-cargo-package--C"></a><code>-C</code> <em>PATH</em></dt>
<dd class="option-desc">Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</dd>


<dt class="option-term" id="option-cargo-package--h"><a class="option-anchor" href="#option-cargo-package--h"></a><code>-h</code></dt>
<dt class="option-term" id="option-cargo-package---help"><a class="option-anchor" href="#option-cargo-package---help"></a><code>--help</code></dt>
<dd class="option-desc">Prints help information.</dd>


<dt class="option-term" id="option-cargo-package--Z"><a class="option-anchor" href="#option-cargo-package--Z"></a><code>-Z</code> <em>flag</em></dt>
<dd class="option-desc">Unstable (nightly-only) flags to Cargo. Run <code>cargo -Z help</code> for details.</dd>


</dl>


## ENVIRONMENT

See [the reference](../reference/environment-variables.html) for
details on environment variables that Cargo reads.


## EXIT STATUS

* `0`: Cargo succeeded.
* `101`: Cargo failed to complete.


## EXAMPLES

1. Create a compressed `.crate` file of the current package:

       cargo package

## SEE ALSO
[cargo(1)](cargo.html), [cargo-publish(1)](cargo-publish.html)
