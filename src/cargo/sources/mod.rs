//! The trait for sources of Cargo packages and its built-in implementations.
//!
//! A source is a provider that contains source files and metadata of packages.
//! It provides a number of methods to fetch those package information, for
//! example, querying metadata or downloading files for a package. These
//! information then can be used as dependencies for other Cargo packages.
//!
//! This module provides [`Source`][source::Source] trait as an abstraction of different sources,
//! as well as [`SourceMap`][source::SourceMap] struct as a map of all available sources.
//!
//! Several built-in implementations of `Source` trait are provided. Namely,
//!
//! * [`RegistrySource`] --- A source that provides an index for people to query
//!   a crate's metadata, and fetch files for a certain crate. crates.io falls
//!   into this category. So do local registry and sparse registry.
//! * [`DirectorySource`] --- Files are downloaded ahead of time. Primarily
//!   designed for crates generated from `cargo vendor`.
//! * [`GitSource`] --- This gets crate information from a git repository.
//! * [`PathSource`] --- This gets crate information from a local path on the
//!   filesystem.
//! * [`ReplacedSource`] --- This manages the [source replacement] feature,
//!   redirecting operations on the original source to the replacement.
//!
//! This module also contains [`SourceConfigMap`], which is effectively the
//! representation of the `[source.*]` value in Cargo configuration.
//!
//! [source replacement]: https://doc.rust-lang.org/nightly/cargo/reference/source-replacement.html

pub use self::config::SourceConfigMap;
pub use self::directory::DirectorySource;
pub use self::git::GitSource;
pub use self::path::PathSource;
pub use self::registry::{RegistrySource, CRATES_IO_DOMAIN, CRATES_IO_INDEX, CRATES_IO_REGISTRY};
pub use self::replaced::ReplacedSource;

pub mod config;
pub mod directory;
pub mod git;
pub mod path;
pub mod registry;
pub mod replaced;
pub mod source;
