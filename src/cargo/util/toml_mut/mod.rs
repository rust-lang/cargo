//! Utilities for in-place editing of Cargo.toml manifests.
//!
//! These utilities operate only on the level of a TOML document, and generally
//! do not perform any processing of information beyond what is required for
//! editing. For more comprehensive usage of manifests, see
//! [`Manifest`](crate::core::manifest::Manifest).
//!
//! In most cases, the entrypoint for editing is
//! [`LocalManifest`](crate::util::toml_mut::manifest::LocalManifest),
//! which contains editing functionality for a given manifest's dependencies.

pub mod dependency;
pub mod manifest;
