//! Utilities for building with rustdoc.

use crate::core::compiler::build_runner::BuildRunner;
use crate::core::compiler::unit::Unit;
use crate::core::compiler::{BuildContext, CompileKind};
use crate::sources::CRATES_IO_REGISTRY;
use crate::util::errors::{CargoResult, internal};
use cargo_util::ProcessBuilder;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::hash;
use url::Url;

const DOCS_RS_URL: &'static str = "https://docs.rs/";

/// Mode used for `std`. This is for unstable feature [`-Zrustdoc-map`][1].
///
/// [1]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#rustdoc-map
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

/// A map of registry names to URLs where documentations are hosted.
/// This is for unstable feature [`-Zrustdoc-map`][1].
///
/// [1]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#rustdoc-map
#[derive(serde::Deserialize, Debug)]
#[serde(default)]
pub struct RustdocExternMap {
    #[serde(deserialize_with = "default_crates_io_to_docs_rs")]
    /// * Key is the registry name in the configuration `[registries.<name>]`.
    /// * Value is the URL where the documentation is hosted.
    registries: HashMap<String, String>,
    std: Option<RustdocExternMode>,
}

impl Default for RustdocExternMap {
    fn default() -> Self {
        Self {
            registries: HashMap::from([(CRATES_IO_REGISTRY.into(), DOCS_RS_URL.into())]),
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

/// Recursively generate html root url for all units and their children.
///
/// This is needed because in case there is a reexport of foreign reexport, you
/// need to have information about grand-children deps level (deps of your deps).
fn build_all_urls(
    build_runner: &BuildRunner<'_, '_>,
    rustdoc: &mut ProcessBuilder,
    unit: &Unit,
    name2url: &HashMap<&String, Url>,
    map: &RustdocExternMap,
    unstable_opts: &mut bool,
    seen: &mut HashSet<Unit>,
) {
    for dep in build_runner.unit_deps(unit) {
        if !seen.insert(dep.unit.clone()) {
            continue;
        }
        if !dep.unit.target.is_linkable() || dep.unit.mode.is_doc() {
            continue;
        }
        for (registry, location) in &map.registries {
            let sid = dep.unit.pkg.package_id().source_id();
            let matches_registry = || -> bool {
                if !sid.is_registry() {
                    return false;
                }
                if sid.is_crates_io() {
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
                *unstable_opts = true;
            }
        }
        build_all_urls(
            build_runner,
            rustdoc,
            &dep.unit,
            name2url,
            map,
            unstable_opts,
            seen,
        );
    }
}

/// Adds unstable flag [`--extern-html-root-url`][1] to the given `rustdoc`
/// invocation. This is for unstable feature [`-Zrustdoc-map`][2].
///
/// [1]: https://doc.rust-lang.org/nightly/rustdoc/unstable-features.html#--extern-html-root-url-control-how-rustdoc-links-to-non-local-crates
/// [2]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#rustdoc-map
pub fn add_root_urls(
    build_runner: &BuildRunner<'_, '_>,
    unit: &Unit,
    rustdoc: &mut ProcessBuilder,
) -> CargoResult<()> {
    let gctx = build_runner.bcx.gctx;
    if !gctx.cli_unstable().rustdoc_map {
        tracing::debug!("`doc.extern-map` ignored, requires -Zrustdoc-map flag");
        return Ok(());
    }
    let map = gctx.doc_extern_map()?;
    let mut unstable_opts = false;
    // Collect mapping of registry name -> index url.
    let name2url: HashMap<&String, Url> = map
        .registries
        .keys()
        .filter_map(|name| {
            if let Ok(index_url) = gctx.get_registry_index(name) {
                Some((name, index_url))
            } else {
                tracing::warn!(
                    "`doc.extern-map.{}` specifies a registry that is not defined",
                    name
                );
                None
            }
        })
        .collect();
    build_all_urls(
        build_runner,
        rustdoc,
        unit,
        &name2url,
        map,
        &mut unstable_opts,
        &mut HashSet::new(),
    );
    let std_url = match &map.std {
        None | Some(RustdocExternMode::Remote) => None,
        Some(RustdocExternMode::Local) => {
            let sysroot = &build_runner.bcx.target_data.info(CompileKind::Host).sysroot;
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
                tracing::warn!(
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

/// Adds unstable flag [`--output-format`][1] to the given `rustdoc`
/// invocation. This is for unstable feature `-Zunstable-features`.
///
/// [1]: https://doc.rust-lang.org/nightly/rustdoc/unstable-features.html?highlight=output-format#-w--output-format-output-format
pub fn add_output_format(
    build_runner: &BuildRunner<'_, '_>,
    rustdoc: &mut ProcessBuilder,
) -> CargoResult<()> {
    let gctx = build_runner.bcx.gctx;
    if !gctx.cli_unstable().unstable_options {
        tracing::debug!("`unstable-options` is ignored, required -Zunstable-options flag");
        return Ok(());
    }

    if build_runner.bcx.build_config.intent.wants_doc_json_output() {
        rustdoc.arg("-Zunstable-options");
        rustdoc.arg("--output-format=json");
    }

    Ok(())
}

/// Indicates whether a target should have examples scraped from it by rustdoc.
/// Configured within Cargo.toml and only for unstable feature
/// [`-Zrustdoc-scrape-examples`][1].
///
/// [1]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#scrape-examples
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug, Copy)]
pub enum RustdocScrapeExamples {
    Enabled,
    Disabled,
    Unset,
}

impl RustdocScrapeExamples {
    pub fn is_enabled(&self) -> bool {
        matches!(self, RustdocScrapeExamples::Enabled)
    }

    pub fn is_unset(&self) -> bool {
        matches!(self, RustdocScrapeExamples::Unset)
    }
}

impl BuildContext<'_, '_> {
    /// Returns the set of [`Docscrape`] units that have a direct dependency on `unit`.
    ///
    /// [`RunCustomBuild`] units are excluded because we allow failures
    /// from type checks but not build script executions.
    /// A plain old `cargo doc` would just die if a build script execution fails,
    /// there is no reason for `-Zrustdoc-scrape-examples` to keep going.
    ///
    /// [`Docscrape`]: crate::core::compiler::CompileMode::Docscrape
    /// [`RunCustomBuild`]: crate::core::compiler::CompileMode::Docscrape
    pub fn scrape_units_have_dep_on<'a>(&'a self, unit: &'a Unit) -> Vec<&'a Unit> {
        self.scrape_units
            .iter()
            .filter(|scrape_unit| {
                self.unit_graph[scrape_unit]
                    .iter()
                    .any(|dep| &dep.unit == unit && !dep.unit.mode.is_run_custom_build())
            })
            .collect()
    }

    /// Returns true if this unit is needed for doing doc-scraping and is also
    /// allowed to fail without killing the build.
    pub fn unit_can_fail_for_docscraping(&self, unit: &Unit) -> bool {
        // If the unit is not a Docscrape unit, e.g. a Lib target that is
        // checked to scrape an Example target, then we need to get the doc-scrape-examples
        // configuration for the reverse-dependent Example target.
        let for_scrape_units = if unit.mode.is_doc_scrape() {
            vec![unit]
        } else {
            self.scrape_units_have_dep_on(unit)
        };

        if for_scrape_units.is_empty() {
            false
        } else {
            // All Docscrape units must have doc-scrape-examples unset. If any are true,
            // then the unit is not allowed to fail.
            for_scrape_units
                .iter()
                .all(|unit| unit.target.doc_scrape_examples().is_unset())
        }
    }
}
