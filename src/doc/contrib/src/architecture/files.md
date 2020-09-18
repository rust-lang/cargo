# Files

This chapter gives some pointers on where to start looking at Cargo's on-disk
data file structures.

* [`Layout`] is the abstraction for the `target` directory. It handles locking
  the target directory, and providing paths to the parts inside. There is a
  separate `Layout` for each "target".
* [`Resolve`] contains the contents of the `Cargo.lock` file. See the [`encode`]
  module for the different `Cargo.lock` formats.
* [`TomlManifest`] contains the contents of the `Cargo.toml` file. It is translated
  to a [`Manifest`] object for some simplification, and the `Manifest` is stored
  in a [`Package`].
* The [`fingerprint`] module deals with the fingerprint information stored in
  `target/debug/.fingerprint`. This tracks whether or not a crate needs to be
  rebuilt.
* `cargo install` tracks its installed files with some metadata in
  `$CARGO_HOME`. The metadata is managed in the
  [`common_for_install_and_uninstall`] module.
* Git sources are cached in `$CARGO_HOME/git`. The code for this cache is in
  the [`git`] source module.
* Registries are cached in `$CARGO_HOME/registry`. There are three parts, the
  index, the compressed `.crate` files, and the extracted sources of those
  crate files.
    * Management of the registry cache can be found in the [`registry`] source
      module. Note that this includes an on-disk cache as an optimization for
      accessing the git repository.
    * Saving of `.crate` files is handled by the [`RemoteRegistry`].
    * Extraction of `.crate` files is handled by the [`RegistrySource`].
    * There is a lock for the package cache. Code must be careful, because
      this lock must be obtained manually. See
      [`Config::acquire_package_cache_lock`].

[`Layout`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/layout.rs
[`Resolve`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/resolver/resolve.rs
[`encode`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/resolver/encode.rs
[`TomlManifest`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/util/toml/mod.rs
[`Manifest`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/manifest.rs
[`Package`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/package.rs
[`common_for_install_and_uninstall`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/ops/common_for_install_and_uninstall.rs
[`git`]: https://github.com/rust-lang/cargo/tree/master/src/cargo/sources/git
[`registry`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/sources/registry/mod.rs
[`RemoteRegistry`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/sources/registry/remote.rs
[`RegistrySource`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/sources/registry/mod.rs
[`Config::acquire_package_cache_lock`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/util/config/mod.rs#L1261-L1266

## Filesystems

Cargo tends to get run on a very wide array of file systems. Different file
systems can have a wide range of capabilities, and Cargo should strive to do
its best to handle them. Some examples of issues to deal with:

* Not all file systems support locking. Cargo tries to detect if locking is
  supported, and if not, will ignore lock errors. This isn't ideal, but it is
  difficult to deal with.
* The [`fs::canonicalize`] function doesn't work on all file systems
  (particularly some Windows file systems). If that function is used, there
  should be a fallback if it fails. This function will also return `\\?\`
  style paths on Windows, which can have some issues (such as some tools not
  supporting them, or having issues with relative paths).
* Timestamps can be unreliable. The [`fingerprint`] module has a deeper
  discussion of this. One example is that Docker cache layers will erase the
  fractional part of the time stamp.
* Symlinks are not always supported, particularly on Windows.

[`fingerprint`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/fingerprint.rs
[`fs::canonicalize`]: https://doc.rust-lang.org/std/fs/fn.canonicalize.html
