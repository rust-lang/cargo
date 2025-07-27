use crate::manifest::RustVersion;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::BTreeMap};

/// A single line in the index representing a single version of a package.
#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct IndexPackage<'a> {
    /// Name of the package.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// The version of this dependency.
    pub vers: Version,
    /// All kinds of direct dependencies of the package, including dev and
    /// build dependencies.
    #[serde(borrow)]
    pub deps: Vec<RegistryDependency<'a>>,
    /// Set of features defined for the package, i.e., `[features]` table.
    #[serde(default)]
    pub features: BTreeMap<Cow<'a, str>, Vec<Cow<'a, str>>>,
    /// This field contains features with new, extended syntax. Specifically,
    /// namespaced features (`dep:`) and weak dependencies (`pkg?/feat`).
    ///
    /// This is separated from `features` because versions older than 1.19
    /// will fail to load due to not being able to parse the new syntax, even
    /// with a `Cargo.lock` file.
    pub features2: Option<BTreeMap<Cow<'a, str>, Vec<Cow<'a, str>>>>,
    /// Checksum for verifying the integrity of the corresponding downloaded package.
    pub cksum: String,
    /// If `true`, Cargo will skip this version when resolving.
    ///
    /// This was added in 2014. Everything in the crates.io index has this set
    /// now, so this probably doesn't need to be an option anymore.
    pub yanked: Option<bool>,
    /// Native library name this package links to.
    ///
    /// Added early 2018 (see <https://github.com/rust-lang/cargo/pull/4978>),
    /// can be `None` if published before then.
    pub links: Option<Cow<'a, str>>,
    /// Required version of rust
    ///
    /// Corresponds to `package.rust-version`.
    ///
    /// Added in 2023 (see <https://github.com/rust-lang/crates.io/pull/6267>),
    /// can be `None` if published before then or if not set in the manifest.
    #[cfg_attr(feature = "unstable-schema", schemars(with = "Option<String>"))]
    pub rust_version: Option<RustVersion>,
    /// The schema version for this entry.
    ///
    /// If this is None, it defaults to version `1`. Entries with unknown
    /// versions are ignored.
    ///
    /// Version `2` schema adds the `features2` field.
    ///
    /// Version `3` schema adds `artifact`, `bindep_targes`, and `lib` for
    /// artifact dependencies support.
    ///
    /// This provides a method to safely introduce changes to index entries
    /// and allow older versions of cargo to ignore newer entries it doesn't
    /// understand. This is honored as of 1.51, so unfortunately older
    /// versions will ignore it, and potentially misinterpret version 2 and
    /// newer entries.
    ///
    /// The intent is that versions older than 1.51 will work with a
    /// pre-existing `Cargo.lock`, but they may not correctly process `cargo
    /// update` or build a lock from scratch. In that case, cargo may
    /// incorrectly select a new package that uses a new index schema. A
    /// workaround is to downgrade any packages that are incompatible with the
    /// `--precise` flag of `cargo update`.
    pub v: Option<u32>,
}

/// A dependency as encoded in the [`IndexPackage`] index JSON.
#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct RegistryDependency<'a> {
    /// Name of the dependency. If the dependency is renamed, the original
    /// would be stored in [`RegistryDependency::package`].
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// The SemVer requirement for this dependency.
    #[serde(borrow)]
    pub req: Cow<'a, str>,
    /// Set of features enabled for this dependency.
    #[serde(default)]
    pub features: Vec<Cow<'a, str>>,
    /// Whether or not this is an optional dependency.
    #[serde(default)]
    pub optional: bool,
    /// Whether or not default features are enabled.
    #[serde(default = "default_true")]
    pub default_features: bool,
    /// The target platform for this dependency.
    pub target: Option<Cow<'a, str>>,
    /// The dependency kind. "dev", "build", and "normal".
    pub kind: Option<Cow<'a, str>>,
    /// The URL of the index of the registry where this dependency is from.
    /// `None` if it is from the same index.
    pub registry: Option<Cow<'a, str>>,
    /// The original name if the dependency is renamed.
    pub package: Option<Cow<'a, str>>,
    /// Whether or not this is a public dependency. Unstable. See [RFC 1977].
    ///
    /// [RFC 1977]: https://rust-lang.github.io/rfcs/1977-public-private-dependencies.html
    pub public: Option<bool>,
    /// The artifacts to build from this dependency.
    pub artifact: Option<Vec<Cow<'a, str>>>,
    /// The target for bindep.
    pub bindep_target: Option<Cow<'a, str>>,
    /// Whether or not this is a library dependency.
    #[serde(default)]
    pub lib: bool,
}

fn default_true() -> bool {
    true
}

#[test]
fn escaped_char_in_index_json_blob() {
    let _: IndexPackage<'_> = serde_json::from_str(
        r#"{"name":"a","vers":"0.0.1","deps":[],"cksum":"bae3","features":{}}"#,
    )
    .unwrap();
    let _: IndexPackage<'_> = serde_json::from_str(
        r#"{"name":"a","vers":"0.0.1","deps":[],"cksum":"bae3","features":{"test":["k","q"]},"links":"a-sys"}"#
    ).unwrap();

    // Now we add escaped cher all the places they can go
    // these are not valid, but it should error later than json parsing
    let _: IndexPackage<'_> = serde_json::from_str(
        r#"{
        "name":"This name has a escaped cher in it \n\t\" ",
        "vers":"0.0.1",
        "deps":[{
            "name": " \n\t\" ",
            "req": " \n\t\" ",
            "features": [" \n\t\" "],
            "optional": true,
            "default_features": true,
            "target": " \n\t\" ",
            "kind": " \n\t\" ",
            "registry": " \n\t\" "
        }],
        "cksum":"bae3",
        "features":{"test \n\t\" ":["k \n\t\" ","q \n\t\" "]},
        "links":" \n\t\" "}"#,
    )
    .unwrap();
}

#[cfg(feature = "unstable-schema")]
#[test]
fn dump_index_schema() {
    let schema = schemars::schema_for!(crate::index::IndexPackage<'_>);
    let dump = serde_json::to_string_pretty(&schema).unwrap();
    snapbox::assert_data_eq!(dump, snapbox::file!("../index.schema.json").raw());
}
