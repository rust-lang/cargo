# Cargo Home

The "Cargo home" functions as a download and source cache.
When building a [crate][def-crate], Cargo stores downloaded build dependencies in the Cargo home.
You can alter the location of the Cargo home by setting the `CARGO_HOME` [environmental variable][env].
The [home](https://crates.io/crates/home) crate provides an API for getting this location if you need this information inside your Rust crate.
By default, the Cargo home is located in `$HOME/.cargo/`.

Please note that the internal structure of the Cargo home is not stabilized and may be subject to change at any time.

The Cargo home consists of following components:

## Files:

* `config.toml`
	Cargo's global configuration file, see the [config entry in the reference][config].

* `credentials.toml`
 	Private login credentials from [`cargo login`] in order to log in to a [registry][def-registry].

* `.crates.toml`, `.crates2.json`
	These hidden files contain [package][def-package] information of crates installed via [`cargo install`]. Do NOT edit by hand!

## Directories:

* `bin`
The bin directory contains executables of crates that were installed via [`cargo install`] or [`rustup`](https://rust-lang.github.io/rustup/).
To be able to make these binaries accessible, add the path of the directory to your `$PATH` environment variable.

 *  `git`
	Git sources are stored here:

    * `git/db`
		When a crate depends on a git repository, Cargo clones the repo as a bare repo into this directory and updates it if necessary.

    * `git/checkouts`
		If a git source is used, the required commit of the repo is checked out from the bare repo inside `git/db` into this directory.
		This provides the compiler with the actual files contained in the repo of the commit specified for that dependency.
		Multiple checkouts of different commits of the same repo are possible.

* `registry`
	Packages and metadata of crate registries (such as [crates.io](https://crates.io/)) are located here.

  * `registry/index`
		The index is a bare git repository which contains the metadata (versions, dependencies etc) of all available crates of a registry.

  *  `registry/cache`
		Downloaded dependencies are stored in the cache. The crates are compressed gzip archives named with a `.crate` extension.

  * `registry/src`
		If a downloaded `.crate` archive is required by a package, it is unpacked into `registry/src` folder where rustc will find the `.rs` files.


## Caching the Cargo home in CI

To avoid redownloading all crate dependencies during continuous integration, you can cache the `$CARGO_HOME` directory.
However, caching the entire directory is often inefficient as it will contain downloaded sources twice.
If we depend on a crate such as `serde 1.0.92` and cache the entire `$CARGO_HOME` we would actually cache the sources twice, the `serde-1.0.92.crate` inside `registry/cache` and the extracted `.rs` files of serde inside `registry/src`.
That can unnecessarily slow down the build as downloading, extracting, recompressing and reuploading the cache to the CI servers can take some time.

If you wish to cache binaries installed with [`cargo install`], you need to cache the `bin/` folder and the `.crates.toml` and `.crates2.json` files.

It should be sufficient to cache the following files and directories across builds:

* `.crates.toml`
* `.crates2.json`
* `bin/`
* `registry/index/`
* `registry/cache/`
* `git/db/`



## Vendoring all dependencies of a project

See the [`cargo vendor`] subcommand.



## Clearing the cache

In theory, you can always remove any part of the cache and Cargo will do its best to restore sources if a crate needs them either by reextracting an archive or checking out a bare repo or by simply redownloading the sources from the web.

Alternatively, the [cargo-cache](https://crates.io/crates/cargo-cache) crate provides a simple CLI tool to only clear selected parts of the cache or show sizes of its components in your command-line.

[`cargo install`]: ../commands/cargo-install.md
[`cargo login`]: ../commands/cargo-login.md
[`cargo vendor`]: ../commands/cargo-vendor.md
[config]: ../reference/config.md
[def-crate]:     ../appendix/glossary.md#crate     '"crate" (glossary entry)'
[def-package]:   ../appendix/glossary.md#package   '"package" (glossary entry)'
[def-registry]:  ../appendix/glossary.md#registry  '"registry" (glossary entry)'
[env]: ../reference/environment-variables.md
