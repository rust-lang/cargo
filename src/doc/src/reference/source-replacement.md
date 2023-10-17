# Source Replacement

This document is about replacing the crate index. You can read about overriding
dependencies in the [overriding dependencies] section of this
documentation.

A *source* is a provider that contains crates that may be included as
dependencies for a package. Cargo supports the ability to **replace one source
with another** to express strategies such as:

* Vendoring --- custom sources can be defined which represent crates on the local
  filesystem. These sources are subsets of the source that they're replacing and
  can be checked into packages if necessary.

* Mirroring --- sources can be replaced with an equivalent version which acts as a
  cache for crates.io itself.

Cargo has a core assumption about source replacement that the source code is
exactly the same from both sources. Note that this also means that
a replacement source is not allowed to have crates which are not present in the
original source.

As a consequence, source replacement is not appropriate for situations such as
patching a dependency or a private registry. Cargo supports patching
dependencies through the usage of [the `[patch]` key][overriding
dependencies], and private registry support is described in [the Registries
chapter][registries].

When using source replacement, running commands like `cargo publish` that need to
contact the registry require passing the `--registry` option. This helps avoid
any ambiguity about which registry to contact, and will use the authentication
token for the specified registry.

[overriding dependencies]: overriding-dependencies.md
[registries]: registries.md

## Configuration

Configuration of replacement sources is done through [`.cargo/config.toml`][config]
and the full set of available keys are:

```toml
# The `source` table is where all keys related to source-replacement
# are stored.
[source]

# Under the `source` table are a number of other tables whose keys are a
# name for the relevant source. For example this section defines a new
# source, called `my-vendor-source`, which comes from a directory
# located at `vendor` relative to the directory containing this `.cargo/config.toml`
# file
[source.my-vendor-source]
directory = "vendor"

# The crates.io default source for crates is available under the name
# "crates-io", and here we use the `replace-with` key to indicate that it's
# replaced with our source above.
#
# The `replace-with` key can also reference an alternative registry name
# defined in the `[registries]` table.
[source.crates-io]
replace-with = "my-vendor-source"

# Each source has its own table where the key is the name of the source
[source.the-source-name]

# Indicate that `the-source-name` will be replaced with `another-source`,
# defined elsewhere
replace-with = "another-source"

# Several kinds of sources can be specified (described in more detail below):
registry = "https://example.com/path/to/index"
local-registry = "path/to/registry"
directory = "path/to/vendor"

# Git sources can optionally specify a branch/tag/rev as well
git = "https://example.com/path/to/repo"
# branch = "master"
# tag = "v1.0.1"
# rev = "313f44e8"
```

[config]: config.md

## Registry Sources

A "registry source" is one that is the same as crates.io itself. That is, it has
an index served in a git repository which matches the format of the
[crates.io index](https://github.com/rust-lang/crates.io-index). That repository
then has configuration indicating where to download crates from.

Currently there is not an already-available project for setting up a mirror of
crates.io. Stay tuned though!

## Local Registry Sources

A "local registry source" is intended to be a subset of another registry
source, but available on the local filesystem (aka vendoring). Local registries
are downloaded ahead of time, typically sync'd with a `Cargo.lock`, and are
made up of a set of `*.crate` files and an index like the normal registry is.

The primary way to manage and create local registry sources is through the
[`cargo-local-registry`][cargo-local-registry] subcommand,
[available on crates.io][cargo-local-registry] and can be installed with
`cargo install cargo-local-registry`.

[cargo-local-registry]: https://crates.io/crates/cargo-local-registry

Local registries are contained within one directory and contain a number of
`*.crate` files downloaded from crates.io as well as an `index` directory with
the same format as the crates.io-index project (populated with just entries for
the crates that are present).

## Directory Sources

A "directory source" is similar to a local registry source where it contains a
number of crates available on the local filesystem, suitable for vendoring
dependencies. Directory sources are primarily managed by the `cargo vendor`
subcommand.

Directory sources are distinct from local registries though in that they contain
the unpacked version of `*.crate` files, making it more suitable in some
situations to check everything into source control. A directory source is just a
directory containing a number of other directories which contain the source code
for crates (the unpacked version of `*.crate` files). Currently no restriction
is placed on the name of each directory.

Each crate in a directory source also has an associated metadata file indicating
the checksum of each file in the crate to protect against accidental
modifications.
