//! Utilities for building with rustdoc.

use crate::core::compiler::context::Context;
use crate::core::compiler::unit::Unit;
use crate::core::compiler::CompileKind;
use crate::sources::CRATES_IO_REGISTRY;
use crate::util::errors::{internal, CargoResult};
use cargo_util::ProcessBuilder;
use std::collections::HashMap;
use std::fmt;
use std::hash;
use url::Url;

const DOCS_RS_URL: &'static str = "https://docs.rs/";

/// Mode used for `std`.
#[derive(Debug, Hash)]
pub enum RustdocExternMode {
    /// Use a local `file://` URL.
    Local,
    /// Use a remote URL to <https://doc.rust-lang.org/> (default).
    Remote,
    /// An arbitrary URL.
    Url(String),
}

impl From<String> for RustdocExternMode {
    fn from(s: String) -> RustdocExternMode {
        match s.as_ref() {
            "local" => RustdocExternMode::Local,
            "remote" => RustdocExternMode::Remote,
            _ => RustdocExternMode::Url(s),
        }
    }
}

impl fmt::Display for RustdocExternMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RustdocExternMode::Local => "local".fmt(f),
            RustdocExternMode::Remote => "remote".fmt(f),
            RustdocExternMode::Url(s) => s.fmt(f),
        }
    }
}

impl<'de> serde::de::Deserialize<'de> for RustdocExternMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.into())
    }
}

#[derive(serde::Deserialize, Debug)]
#[serde(default)]
pub struct RustdocExternMap {
    #[serde(deserialize_with = "default_crates_io_to_docs_rs")]
    pub(crate) registries: HashMap<String, String>,
    std: Option<RustdocExternMode>,
}

impl Default for RustdocExternMap {
    fn default() -> Self {
        let mut registries = HashMap::new();
        registries.insert(CRATES_IO_REGISTRY.into(), DOCS_RS_URL.into());
        Self {
            registries,
            std: None,
        }
    }
}

fn default_crates_io_to_docs_rs<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> Result<HashMap<String, String>, D::Error> {
    use serde::Deserialize;
    let mut registries = HashMap::deserialize(de)?;
    if !registries.contains_key(CRATES_IO_REGISTRY) {
        registries.insert(CRATES_IO_REGISTRY.into(), DOCS_RS_URL.into());
    }
    Ok(registries)
}

impl hash::Hash for RustdocExternMap {
    fn hash<H: hash::Hasher>(&self, into: &mut H) {
        self.std.hash(into);
        for (key, value) in &self.registries {
            key.hash(into);
            value.hash(into);
        }
    }
}

pub fn add_root_urls(
    cx: &Context<'_, '_>,
    unit: &Unit,
    rustdoc: &mut ProcessBuilder,
) -> CargoResult<()> {
    let config = cx.bcx.config;
    if !config.cli_unstable().rustdoc_map {
        log::debug!("`doc.extern-map` ignored, requires -Zrustdoc-map flag");
        return Ok(());
    }
    let map = config.doc_extern_map()?;
    let mut unstable_opts = false;
    // Collect mapping of registry name -> index url.
    let name2url: HashMap<&String, Url> = map
        .registries
        .keys()
        .filter_map(|name| {
            if let Ok(index_url) = config.get_registry_index(name) {
                Some((name, index_url))
            } else {
                log::warn!(
                    "`doc.extern-map.{}` specifies a registry that is not defined",
                    name
                );
                None
            }
        })
        .collect();
    for dep in cx.unit_deps(unit) {
        if dep.unit.target.is_linkable() && !dep.unit.mode.is_doc() {
            for (registry, location) in &map.registries {
                let sid = dep.unit.pkg.package_id().source_id();
                let matches_registry = || -> bool {
                    if !sid.is_registry() {
                        return false;
                    }
                    if sid.is_default_registry() {
                        return registry == CRATES_IO_REGISTRY;
                    }
                    if let Some(index_url) = name2url.get(registry) {
                        return index_url == sid.url();
                    }
                    false
                };
                if matches_registry() {
                    let mut url = location.clone();
                    if !url.contains("{pkg_name}") && !url.contains("{version}") {
                        if !url.ends_with('/') {
                            url.push('/');
                        }
                        url.push_str("{pkg_name}/{version}/");
                    }
                    let url = url
                        .replace("{pkg_name}", &dep.unit.pkg.name())
                        .replace("{version}", &dep.unit.pkg.version().to_string());
                    rustdoc.arg("--extern-html-root-url");
                    rustdoc.arg(format!("{}={}", dep.unit.target.crate_name(), url));
                    unstable_opts = true;
                }
            }
        }
    }
    let std_url = match &map.std {
        None | Some(RustdocExternMode::Remote) => None,
        Some(RustdocExternMode::Local) => {
            let sysroot = &cx.bcx.target_data.info(CompileKind::Host).sysroot;
            let html_root = sysroot.join("share").join("doc").join("rust").join("html");
            if html_root.exists() {
                let url = Url::from_file_path(&html_root).map_err(|()| {
                    internal(format!(
                        "`{}` failed to convert to URL",
                        html_root.display()
                    ))
                })?;
                Some(url.to_string())
            } else {
                log::warn!(
                    "`doc.extern-map.std` is \"local\", but local docs don't appear to exist at {}",
                    html_root.display()
                );
                None
            }
        }
        Some(RustdocExternMode::Url(s)) => Some(s.to_string()),
    };
    if let Some(url) = std_url {
        for name in &["std", "core", "alloc", "proc_macro"] {
            rustdoc.arg("--extern-html-root-url");
            rustdoc.arg(format!("{}={}", name, url));
            unstable_opts = true;
        }
    }

    if unstable_opts {
        rustdoc.arg("-Zunstable-options");
    }
    Ok(())
}
