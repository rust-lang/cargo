## Build cache

Cargo stores the output of a build into the "target" directory. By default,
this is the directory named `target` in the root of your workspace. To change
the location, you can set the `CARGO_TARGET_DIR` [environment variable], the
[`build.target-dir`] config value, or the `--target-dir` command-line flag.

The directory layout depends on whether or not you are cross-compiling for a
different platform with the `--target` flag. When not cross-compiling, the
output goes into the root of the target directory, separated based on whether
or not it is a release build:

Directory | Description
----------|------------
<code style="white-space: nowrap">target/debug/</code> | Contains debug build output.
<code style="white-space: nowrap">target/release/</code> | Contains release build output (with `--release` flag).

When building for another target, the output is placed in a directory with the
name of the target:

Directory | Example
----------|--------
<code style="white-space: nowrap">target/&lt;triple&gt;/debug/</code> | <code style="white-space: nowrap">target/thumbv7em-none-eabihf/debug/</code>
<code style="white-space: nowrap">target/&lt;triple&gt;/release/</code> | <code style="white-space: nowrap">target/thumbv7em-none-eabihf/release/</code>

Within the profile directory (`debug` or `release`), artifacts are placed into
the following directories:

Directory | Description
----------|------------
<code style="white-space: nowrap">target/debug/</code> | Contains the output of the package being built (the `[[bin]]` executables and `[lib]` library targets).
<code style="white-space: nowrap">target/debug/examples/</code> | Contains examples (`[[example]]` targets).

Some commands place their output in dedicated directories in the top level of
the `target` directory:

Directory | Description
----------|------------
<code style="white-space: nowrap">target/doc/</code> | Contains rustdoc documentation ([`cargo doc`]).
<code style="white-space: nowrap">target/package/</code> | Contains the output of the [`cargo package`] and [`cargo publish`] commands.

Cargo also creates several other directories and files needed for the build
process. Their layout is considered internal to Cargo, and is subject to
change. Some of these directories are:

Directory | Description
----------|------------
<code style="white-space: nowrap">target/debug/deps/</code> |  Dependencies and other artifacts.
<code style="white-space: nowrap">target/debug/incremental/</code> |  `rustc` [incremental output], a cache used to speed up subsequent builds.
<code style="white-space: nowrap">target/debug/build/</code> |  Output from [build scripts].

### Shared cache

A third party tool, [sccache], can be used to share built dependencies across
different workspaces.

To setup `sccache`, install it with `cargo install sccache` and set
`RUSTC_WRAPPER` environmental variable to `sccache` before invoking Cargo. If
you use bash, it makes sense to add `export RUSTC_WRAPPER=sccache` to
`.bashrc`. Alternatively, you can set [`build.rustc-wrapper`] in the [Cargo
configuration][config]. Refer to sccache documentation for more details.

[`build.target-dir`]: ../reference/config.md#buildtarget-dir
[`cargo doc`]: ../commands/cargo-doc.md
[`cargo package`]: ../commands/cargo-package.md
[`cargo publish`]: ../commands/cargo-publish.md
[build scripts]: ../reference/build-scripts.md
[config]: ../reference/config.md
[environment variable]: ../reference/environment-variables.md
[incremental output]: ../reference/profiles.md#incremental
[sccache]: https://github.com/mozilla/sccache
