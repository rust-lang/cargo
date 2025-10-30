# cargo-package(1)
## NAME

cargo-package --- Assemble the local package into a distributable tarball

## SYNOPSIS

`cargo package` [_options_]

## DESCRIPTION

This command will create a distributable, compressed `.crate` file with the
source code of the package in the current directory. The resulting file will be
stored in the `target/package` directory. This performs the following steps:

1. Load and check the current workspace, performing some basic checks.
    - Path dependencies are not allowed unless they have a version key. Cargo
      will ignore the path key for dependencies in published packages.
      `dev-dependencies` do not have this restriction.
2. Create the compressed `.crate` file.
    - The original `Cargo.toml` file is rewritten and normalized.
    - `[patch]`, `[replace]`, and `[workspace]` sections are removed from the
      manifest.
    - `Cargo.lock` is always included. When missing, a new lock file will be
      generated unless the `--exclude-lockfile` flag is used. [cargo-install(1)](cargo-install.html)
      will use the packaged lock file if the `--locked` flag is used.
    - A `.cargo_vcs_info.json` file is included that contains information
      about the current VCS checkout hash if available, as well as a flag if the
      worktree is dirty.
    - Symlinks are flattened to their target files.
    - Files and directories are included or excluded based on rules mentioned in
      [the `[include]` and `[exclude]` fields](../reference/manifest.html#the-exclude-and-include-fields).

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
   "sha1": "aac20b6e7e543e6dd4118b246c77225e3a3a1302",
   "dirty": true
 },
 "path_in_vcs": ""
}
```

`dirty` indicates that the Git worktree was dirty when the package
was built.

`path_in_vcs` will be set to a repo-relative path for packages
in subdirectories of the version control repository.

The compatibility of this file is maintained under the same policy
as the JSON output of [cargo-metadata(1)](cargo-metadata.html).

Note that this file provides a best-effort snapshot of the VCS information.
However, the provenance of the package is not verified.
There is no guarantee that the source code in the tarball matches the VCS information.

## OPTIONS

### Package Options

<dl>

<dt class="option-term" id="option-cargo-package--l"><a class="option-anchor" href="#option-cargo-package--l"><code>-l</code></a></dt>
<dt class="option-term" id="option-cargo-package---list"><a class="option-anchor" href="#option-cargo-package---list"><code>--list</code></a></dt>
<dd class="option-desc"><p>Print files included in a package without making one.</p>
</dd>


<dt class="option-term" id="option-cargo-package---no-verify"><a class="option-anchor" href="#option-cargo-package---no-verify"><code>--no-verify</code></a></dt>
<dd class="option-desc"><p>Don’t verify the contents by building them.</p>
</dd>


<dt class="option-term" id="option-cargo-package---no-metadata"><a class="option-anchor" href="#option-cargo-package---no-metadata"><code>--no-metadata</code></a></dt>
<dd class="option-desc"><p>Ignore warnings about a lack of human-usable metadata (such as the description
or the license).</p>
</dd>


<dt class="option-term" id="option-cargo-package---allow-dirty"><a class="option-anchor" href="#option-cargo-package---allow-dirty"><code>--allow-dirty</code></a></dt>
<dd class="option-desc"><p>Allow working directories with uncommitted VCS changes to be packaged.</p>
</dd>


<dt class="option-term" id="option-cargo-package---exclude-lockfile"><a class="option-anchor" href="#option-cargo-package---exclude-lockfile"><code>--exclude-lockfile</code></a></dt>
<dd class="option-desc"><p>Don’t include the lock file when packaging.</p>
<p>This flag is not for general use.
Some tools may expect a lock file to be present (e.g. <code>cargo install --locked</code>).
Consider other options before using this.</p>
</dd>


<dt class="option-term" id="option-cargo-package---index"><a class="option-anchor" href="#option-cargo-package---index"><code>--index</code> <em>index</em></a></dt>
<dd class="option-desc"><p>The URL of the registry index to use.</p>
</dd>


<dt class="option-term" id="option-cargo-package---registry"><a class="option-anchor" href="#option-cargo-package---registry"><code>--registry</code> <em>registry</em></a></dt>
<dd class="option-desc"><p>Name of the registry to package for; see <code>cargo publish --help</code> for more details
about configuration of registry names. The packages will not be published
to this registry, but if we are packaging multiple inter-dependent crates,
lock-files will be generated under the assumption that dependencies will be
published to this registry.</p>
</dd>


<dt class="option-term" id="option-cargo-package---message-format"><a class="option-anchor" href="#option-cargo-package---message-format"><code>--message-format</code> <em>fmt</em></a></dt>
<dd class="option-desc"><p>Specifies the output message format.
Currently, it only works with <code>--list</code> and affects the file listing format.
This is unstable and requires <code>-Zunstable-options</code>.
Valid output formats:</p>
<ul>
<li><code>human</code> (default): Display in a file-per-line format.</li>
<li><code>json</code>: Emit machine-readable JSON information about each package.
One package per JSON line (Newline delimited JSON).
<pre><code class="language-javascript">{
  /* The Package ID Spec of the package. */
  "id": "path+file:///home/foo#0.0.0",
  /* Files of this package */
  "files" {
    /* Relative path in the archive file. */
    "Cargo.toml.orig": {
      /* Where the file is from.
         - "generate" for file being generated during packaging
         - "copy" for file being copied from another location.
      */
      "kind": "copy",
      /* For the "copy" kind,
         it is an absolute path to the actual file content.
         For the "generate" kind,
         it is the original file the generated one is based on.
      */
      "path": "/home/foo/Cargo.toml"
    },
    "Cargo.toml": {
      "kind": "generate",
      "path": "/home/foo/Cargo.toml"
    },
    "src/main.rs": {
      "kind": "copy",
      "path": "/home/foo/src/main.rs"
    }
  }
}
</code></pre>
</li>
</ul>
</dd>


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

<dt class="option-term" id="option-cargo-package--p"><a class="option-anchor" href="#option-cargo-package--p"><code>-p</code> <em>spec</em>…</a></dt>
<dt class="option-term" id="option-cargo-package---package"><a class="option-anchor" href="#option-cargo-package---package"><code>--package</code> <em>spec</em>…</a></dt>
<dd class="option-desc"><p>Package only the specified packages. See <a href="cargo-pkgid.html">cargo-pkgid(1)</a> for the
SPEC format. This flag may be specified multiple times and supports common Unix
glob patterns like <code>*</code>, <code>?</code> and <code>[]</code>. However, to avoid your shell accidentally
expanding glob patterns before Cargo handles them, you must use single quotes or
double quotes around each pattern.</p>
</dd>


<dt class="option-term" id="option-cargo-package---workspace"><a class="option-anchor" href="#option-cargo-package---workspace"><code>--workspace</code></a></dt>
<dd class="option-desc"><p>Package all members in the workspace.</p>
</dd>



<dt class="option-term" id="option-cargo-package---exclude"><a class="option-anchor" href="#option-cargo-package---exclude"><code>--exclude</code> <em>SPEC</em>…</a></dt>
<dd class="option-desc"><p>Exclude the specified packages. Must be used in conjunction with the
<code>--workspace</code> flag. This flag may be specified multiple times and supports
common Unix glob patterns like <code>*</code>, <code>?</code> and <code>[]</code>. However, to avoid your shell
accidentally expanding glob patterns before Cargo handles them, you must use
single quotes or double quotes around each pattern.</p>
</dd>


</dl>

### Compilation Options

<dl>

<dt class="option-term" id="option-cargo-package---target"><a class="option-anchor" href="#option-cargo-package---target"><code>--target</code> <em>triple</em></a></dt>
<dd class="option-desc"><p>Package for the specified target architecture. Flag may be specified multiple times. The default is the host architecture. The general format of the triple is
<code>&lt;arch&gt;&lt;sub&gt;-&lt;vendor&gt;-&lt;sys&gt;-&lt;abi&gt;</code>.</p>
<p>Possible values:</p>
<ul>
<li>Any supported target in <code>rustc --print target-list</code>.</li>
<li><code>"host-tuple"</code>, which will internally be substituted by the host’s target. This can be particularly useful if you’re cross-compiling some crates, and don’t want to specify your host’s machine as a target (for instance, an <code>xtask</code> in a shared project that may be worked on by many hosts).</li>
<li>A path to a custom target specification. See <a href="../../rustc/targets/custom.html#custom-target-lookup-path">Custom Target Lookup Path</a> for more information.</li>
</ul>
<p>This may also be specified with the <code>build.target</code> <a href="../reference/config.html">config value</a>.</p>
<p>Note that specifying this flag makes Cargo run in a different mode where the
target artifacts are placed in a separate directory. See the
<a href="../reference/build-cache.html">build cache</a> documentation for more details.</p>
</dd>


<dt class="option-term" id="option-cargo-package---target-dir"><a class="option-anchor" href="#option-cargo-package---target-dir"><code>--target-dir</code> <em>directory</em></a></dt>
<dd class="option-desc"><p>Directory for all generated artifacts and intermediate files. May also be
specified with the <code>CARGO_TARGET_DIR</code> environment variable, or the
<code>build.target-dir</code> <a href="../reference/config.html">config value</a>.
Defaults to <code>target</code> in the root of the workspace.</p>
</dd>


</dl>

### Feature Selection

The feature flags allow you to control which features are enabled. When no
feature options are given, the `default` feature is activated for every
selected package.

See [the features documentation](../reference/features.html#command-line-feature-options)
for more details.

<dl>

<dt class="option-term" id="option-cargo-package--F"><a class="option-anchor" href="#option-cargo-package--F"><code>-F</code> <em>features</em></a></dt>
<dt class="option-term" id="option-cargo-package---features"><a class="option-anchor" href="#option-cargo-package---features"><code>--features</code> <em>features</em></a></dt>
<dd class="option-desc"><p>Space or comma separated list of features to activate. Features of workspace
members may be enabled with <code>package-name/feature-name</code> syntax. This flag may
be specified multiple times, which enables all specified features.</p>
</dd>


<dt class="option-term" id="option-cargo-package---all-features"><a class="option-anchor" href="#option-cargo-package---all-features"><code>--all-features</code></a></dt>
<dd class="option-desc"><p>Activate all available features of all selected packages.</p>
</dd>


<dt class="option-term" id="option-cargo-package---no-default-features"><a class="option-anchor" href="#option-cargo-package---no-default-features"><code>--no-default-features</code></a></dt>
<dd class="option-desc"><p>Do not activate the <code>default</code> feature of the selected packages.</p>
</dd>


</dl>

### Manifest Options

<dl>

<dt class="option-term" id="option-cargo-package---manifest-path"><a class="option-anchor" href="#option-cargo-package---manifest-path"><code>--manifest-path</code> <em>path</em></a></dt>
<dd class="option-desc"><p>Path to the <code>Cargo.toml</code> file. By default, Cargo searches for the
<code>Cargo.toml</code> file in the current directory or any parent directory.</p>
</dd>


<dt class="option-term" id="option-cargo-package---locked"><a class="option-anchor" href="#option-cargo-package---locked"><code>--locked</code></a></dt>
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


<dt class="option-term" id="option-cargo-package---offline"><a class="option-anchor" href="#option-cargo-package---offline"><code>--offline</code></a></dt>
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


<dt class="option-term" id="option-cargo-package---frozen"><a class="option-anchor" href="#option-cargo-package---frozen"><code>--frozen</code></a></dt>
<dd class="option-desc"><p>Equivalent to specifying both <code>--locked</code> and <code>--offline</code>.</p>
</dd>


<dt class="option-term" id="option-cargo-package---lockfile-path"><a class="option-anchor" href="#option-cargo-package---lockfile-path"><code>--lockfile-path</code> <em>PATH</em></a></dt>
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

### Miscellaneous Options

<dl>
<dt class="option-term" id="option-cargo-package--j"><a class="option-anchor" href="#option-cargo-package--j"><code>-j</code> <em>N</em></a></dt>
<dt class="option-term" id="option-cargo-package---jobs"><a class="option-anchor" href="#option-cargo-package---jobs"><code>--jobs</code> <em>N</em></a></dt>
<dd class="option-desc"><p>Number of parallel jobs to run. May also be specified with the
<code>build.jobs</code> <a href="../reference/config.html">config value</a>. Defaults to
the number of logical CPUs. If negative, it sets the maximum number of
parallel jobs to the number of logical CPUs plus provided value. If
a string <code>default</code> is provided, it sets the value back to defaults.
Should not be 0.</p>
</dd>

<dt class="option-term" id="option-cargo-package---keep-going"><a class="option-anchor" href="#option-cargo-package---keep-going"><code>--keep-going</code></a></dt>
<dd class="option-desc"><p>Build as many crates in the dependency graph as possible, rather than aborting
the build on the first one that fails to build.</p>
<p>For example if the current package depends on dependencies <code>fails</code> and <code>works</code>,
one of which fails to build, <code>cargo package -j1</code> may or may not build the
one that succeeds (depending on which one of the two builds Cargo picked to run
first), whereas <code>cargo package -j1 --keep-going</code> would definitely run both
builds, even if the one run first fails.</p>
</dd>

</dl>

### Display Options

<dl>
<dt class="option-term" id="option-cargo-package--v"><a class="option-anchor" href="#option-cargo-package--v"><code>-v</code></a></dt>
<dt class="option-term" id="option-cargo-package---verbose"><a class="option-anchor" href="#option-cargo-package---verbose"><code>--verbose</code></a></dt>
<dd class="option-desc"><p>Use verbose output. May be specified twice for “very verbose” output which
includes extra output such as dependency warnings and build script output.
May also be specified with the <code>term.verbose</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-package--q"><a class="option-anchor" href="#option-cargo-package--q"><code>-q</code></a></dt>
<dt class="option-term" id="option-cargo-package---quiet"><a class="option-anchor" href="#option-cargo-package---quiet"><code>--quiet</code></a></dt>
<dd class="option-desc"><p>Do not print cargo log messages.
May also be specified with the <code>term.quiet</code>
<a href="../reference/config.html">config value</a>.</p>
</dd>


<dt class="option-term" id="option-cargo-package---color"><a class="option-anchor" href="#option-cargo-package---color"><code>--color</code> <em>when</em></a></dt>
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

<dt class="option-term" id="option-cargo-package-+toolchain"><a class="option-anchor" href="#option-cargo-package-+toolchain"><code>+</code><em>toolchain</em></a></dt>
<dd class="option-desc"><p>If Cargo has been installed with rustup, and the first argument to <code>cargo</code>
begins with <code>+</code>, it will be interpreted as a rustup toolchain name (such
as <code>+stable</code> or <code>+nightly</code>).
See the <a href="https://rust-lang.github.io/rustup/overrides.html">rustup documentation</a>
for more information about how toolchain overrides work.</p>
</dd>


<dt class="option-term" id="option-cargo-package---config"><a class="option-anchor" href="#option-cargo-package---config"><code>--config</code> <em>KEY=VALUE</em> or <em>PATH</em></a></dt>
<dd class="option-desc"><p>Overrides a Cargo configuration value. The argument should be in TOML syntax of <code>KEY=VALUE</code>,
or provided as a path to an extra configuration file. This flag may be specified multiple times.
See the <a href="../reference/config.html#command-line-overrides">command-line overrides section</a> for more information.</p>
</dd>


<dt class="option-term" id="option-cargo-package--C"><a class="option-anchor" href="#option-cargo-package--C"><code>-C</code> <em>PATH</em></a></dt>
<dd class="option-desc"><p>Changes the current working directory before executing any specified operations. This affects
things like where cargo looks by default for the project manifest (<code>Cargo.toml</code>), as well as
the directories searched for discovering <code>.cargo/config.toml</code>, for example. This option must
appear before the command name, for example <code>cargo -C path/to/my-project build</code>.</p>
<p>This option is only available on the <a href="https://doc.rust-lang.org/book/appendix-07-nightly-rust.html">nightly
channel</a> and
requires the <code>-Z unstable-options</code> flag to enable (see
<a href="https://github.com/rust-lang/cargo/issues/10098">#10098</a>).</p>
</dd>


<dt class="option-term" id="option-cargo-package--h"><a class="option-anchor" href="#option-cargo-package--h"><code>-h</code></a></dt>
<dt class="option-term" id="option-cargo-package---help"><a class="option-anchor" href="#option-cargo-package---help"><code>--help</code></a></dt>
<dd class="option-desc"><p>Prints help information.</p>
</dd>


<dt class="option-term" id="option-cargo-package--Z"><a class="option-anchor" href="#option-cargo-package--Z"><code>-Z</code> <em>flag</em></a></dt>
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

1. Create a compressed `.crate` file of the current package:

       cargo package

## SEE ALSO
[cargo(1)](cargo.html), [cargo-publish(1)](cargo-publish.html)
