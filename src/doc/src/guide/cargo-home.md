## Cargo Home

The cargo home functions as a download and source cache.
When building a crate, cargo stores downloaded build dependencies in the cargo home.
You can alter the location of the cargo home by setting the `CARGO_HOME` [environmental variable](../reference/environment-variables.html).
The [home](https://crates.io/crates/home) crate provides an api for getting this location if you need this information inside your rust crate.
By default, the cargo home is located in `${HOME}/.cargo/`.

Please note that the internal structure of the cargo home is not stabilized and may be subject to change at any time.

The cargo home consists of following components:

## Files:

* `config`
	Cargos global configuration file, see the [config entry in the reference](../reference/config.html).

* `credentials`
 	Private login credentials from [`cargo login`](../commands/cargo-login.html) in order to login into a registry.

* `.crates.toml`
	This hidden file contains package information of crates installed via [cargo install](../commands/cargo-install.html). Do NOT edit by hand!

## Directories:

* `bin`
The bin directory contains executables of crates that were installed via "cargo install" or `rustup`.
To be able to make these binaries accessible, add the path of the directory to your `${PATH}`.

 *  `git`
	Git sources are stored here:

    * `git/db`
		When a crate depends on a git repository, cargo clones the repo as a bare repo into this directory and updates it if neccessary.

    * `git/checkouts`
		If a git source is used, the required commit of the repo is checked out from the bare repo inside `git/db` into this directory.
		This provides the compiler with the actual files contained in the repo of the commit specified for that dependency.
		Multiple checkouts of different commits of the same repo are possible.

* `registry`
	Packages and metadata of crate registries (such as crates.io) are located here.

  * `registry/index`
		The index is a bare git repository which contains the metadata (versions, dependencies etc) of all available crates of a registry.

  *  `registry/cache`
		Downloaded dependencies are stored in the cache. The crates are compressed gzip archives named with a `.crate` extension.

  * `registry/src`
		If a downloaded `.crate` archive is required by a package, it is unpacked into `registry/src` folder where rustc will find the `.rs` files.


## Caching the cargo home in CI

To avoid redownloading all crate dependencies during continuous integration, you can cache the `$CARGO_HOME` directory.
However, caching the entire directory as is is often inefficient as it will contain downloaded sources twice.
If we depend on `cargo 0.38.0` and cache the entire `$CARGO_HOME` we would actually cache the sources twice, the `cargo-0.38.0.crate` inside `registry/cache` and the extracted `.rs` files of cargoinside `registry/src`.
The can unneccessarily slow down the build as downloading, extracting, recompressing and reuploading the cache ot the CI servers can take some time.

It should be sufficient to only cache the following directories across builds:

* `bin/`
* `registry/index/`
* `registry/cache/`
* `git/db/`



## Vendoring all dependencies of a project

See the [cargo vendor](commands/cargo-vendor.md) subcommand.



## Clearing the cache

In theory, you can always remove any part of the cache and cargo will do its best to restore sources if a crate needs them either by reextracting an archive or checking out a bare repo or by simply redownloading the sources from the web.

Alternatively, the [cargo-cache](https://crates.io/crates/cargo-cache) crate provides a simple CLI tool to only clear selected parts of the cache or show sizes of its components in your commandline.
