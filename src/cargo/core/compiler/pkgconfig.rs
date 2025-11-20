//! Support for pkg-config dependencies declared in Cargo.toml
//!
//! This module handles querying system pkg-config for dependencies declared
//! in the `[pkgconfig-dependencies]` section and generating metadata.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context as _};
use cargo_util_schemas::manifest::TomlPkgConfigDependency;

use crate::util::errors::CargoResult;

/// Information about a resolved pkg-config dependency
#[derive(Debug, Clone)]
pub struct PkgConfigLibrary {
    /// The name used to query pkg-config
    pub name: String,
    /// Version string from pkg-config, or empty if not available
    pub version: String,
    /// How the dependency was resolved (pkg-config, fallback, not-found, etc.)
    pub resolved_via: ResolutionMethod,
    /// Include paths (-I flags)
    pub include_paths: Vec<String>,
    /// Library search paths (-L flags)
    pub lib_paths: Vec<String>,
    /// Library names to link (-l flags)
    pub libs: Vec<String>,
    /// Other compiler flags (excluding -I and -D)
    pub cflags: Vec<String>,
    /// Preprocessor defines (-D flags)
    pub defines: Vec<String>,
    /// Other linker flags (excluding -L and -l)
    pub ldflags: Vec<String>,
    /// Raw output from pkg-config --cflags
    pub raw_cflags: String,
    /// Raw output from pkg-config --libs
    pub raw_ldflags: String,
}

/// How a pkg-config dependency was resolved
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionMethod {
    /// Successfully found via pkg-config
    PkgConfig,
    /// Used fallback specification
    Fallback,
    /// Not found and optional=true
    NotFound,
    /// Not probed because feature was not enabled
    NotProbed,
}

impl ResolutionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionMethod::PkgConfig => "pkg-config",
            ResolutionMethod::Fallback => "fallback",
            ResolutionMethod::NotFound => "not-found",
            ResolutionMethod::NotProbed => "not-probed",
        }
    }
}

/// Parse pkg-config output and extract flags
fn parse_pkg_config_output(
    cflags_output: &str,
    ldflags_output: &str,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut include_paths = Vec::new();
    let mut lib_paths = Vec::new();
    let mut libs = Vec::new();
    let mut cflags = Vec::new();
    let mut defines = Vec::new();

    // Parse cflags
    for flag in cflags_output.split_whitespace() {
        if flag.starts_with("-I") {
            include_paths.push(flag[2..].to_string());
        } else if flag.starts_with("-D") {
            defines.push(flag[2..].to_string());
        } else if !flag.is_empty() {
            cflags.push(flag.to_string());
        }
    }

    // Parse ldflags
    let mut ldflags_collected = Vec::new();
    for flag in ldflags_output.split_whitespace() {
        if flag.starts_with("-L") {
            lib_paths.push(flag[2..].to_string());
        } else if flag.starts_with("-l") {
            libs.push(flag[2..].to_string());
        } else if !flag.is_empty() {
            ldflags_collected.push(flag.to_string());
        }
    }

    (include_paths, lib_paths, libs, cflags, defines)
}

/// Sanitize a package name to be a valid Rust module name
pub fn sanitize_module_name(name: &str) -> String {
    let mut result = String::new();

    // If starts with digit, prepend "lib_"
    if name.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        result.push_str("lib_");
    }

    for ch in name.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => result.push(ch),
            '-' | '.' | '+' => result.push('_'),
            _ => {
                // Skip other characters or convert to underscore
                result.push('_');
            }
        }
    }

    result.to_lowercase()
}

/// Generate Rust code for a single dependency's metadata
fn generate_dep_module(name: &str, lib: &PkgConfigLibrary) -> String {
    let module_name = sanitize_module_name(name);

    // Format arrays for Rust code
    let format_str_array = |items: &[String]| -> String {
        let strs: Vec<_> = items.iter().map(|s| format!("\"{}\"", s)).collect();
        format!("&[{}]", strs.join(", "))
    };

    format!(
        r#"    /// Metadata for pkgconfig dependency: {}
    pub mod {} {{
        /// Version from pkg-config --modversion
        pub const VERSION: &str = "{}";

        /// Successfully resolved
        pub const FOUND: bool = {};

        /// Resolution method
        pub const RESOLVED_VIA: &str = "{}";

        /// Include paths from --cflags-only-I
        pub const INCLUDE_PATHS: &[&str] = &{:?};

        /// Library paths from --libs-only-L
        pub const LIB_PATHS: &[&str] = &{:?};

        /// Libraries from --libs-only-l
        pub const LIBS: &[&str] = &{:?};

        /// Other compiler flags from --cflags-only-other
        pub const CFLAGS: &[&str] = &{:?};

        /// Defines from -D flags
        pub const DEFINES: &[&str] = &{:?};

        /// Other linker flags from --libs-only-other
        pub const LDFLAGS: &[&str] = &{:?};

        /// Raw pkg-config --cflags output
        pub const RAW_CFLAGS: &str = "{}";

        /// Raw pkg-config --libs output
        pub const RAW_LDFLAGS: &str = "{}";
    }}
"#,
        name,
        module_name,
        lib.version,
        lib.resolved_via != ResolutionMethod::NotFound
            && lib.resolved_via != ResolutionMethod::NotProbed,
        lib.resolved_via.as_str(),
        lib.include_paths
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        lib.lib_paths
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        lib.libs.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        lib.cflags
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        lib.defines
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        lib.ldflags
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        lib.raw_cflags.replace('\\', "\\\\").replace('"', "\\\""),
        lib.raw_ldflags.replace('\\', "\\\\").replace('"', "\\\""),
    )
}

/// Generate the pkgconfig_meta.rs file content
pub fn generate_metadata_file(libraries: &BTreeMap<String, PkgConfigLibrary>) -> String {
    let mut modules = String::new();

    for (name, lib) in libraries.iter() {
        modules.push_str(&generate_dep_module(name, lib));
    }

    format!(
        r#"// Auto-generated by Cargo from [pkgconfig-dependencies]
// DO NOT EDIT - regenerated on each build

#![allow(dead_code, non_upper_case_globals)]

/// Package metadata for pkgconfig-dependencies
pub mod pkgconfig {{
{}}}
"#,
        modules
    )
}

/// Query pkg-config for a single dependency
pub fn query_pkg_config(
    name: &str,
    version_constraint: &str,
) -> CargoResult<PkgConfigLibrary> {
    let mut config = pkg_config::Config::new();

    // Set version constraint
    config.atleast_version(version_constraint);

    match config.probe(name) {
        Ok(lib) => {
            let include_paths = lib
                .include_paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>();

            let lib_paths = lib
                .link_paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>();

            let libs = lib.libs.clone();

            // Reconstruct raw cflags and ldflags for debugging output
            let raw_cflags = {
                let mut parts = Vec::new();
                for path in &include_paths {
                    parts.push(format!("-I{}", path));
                }
                // Note: We don't have access to original defines in the library struct
                parts.join(" ")
            };

            let raw_ldflags = {
                let mut parts = Vec::new();
                for path in &lib_paths {
                    parts.push(format!("-L{}", path));
                }
                for lib in &libs {
                    parts.push(format!("-l{}", lib));
                }
                parts.join(" ")
            };

            Ok(PkgConfigLibrary {
                name: name.to_string(),
                version: lib.version.clone(),
                resolved_via: ResolutionMethod::PkgConfig,
                include_paths,
                lib_paths,
                libs,
                cflags: Vec::new(),
                defines: Vec::new(),
                ldflags: Vec::new(),
                raw_cflags,
                raw_ldflags,
            })
        }
        Err(e) => {
            bail!(
                "pkg-config dependency `{} {}` not found: {}",
                name,
                version_constraint,
                e
            )
        }
    }
}

/// Probe all pkgconfig dependencies for a package
pub fn probe_all_dependencies(
    deps: &BTreeMap<String, TomlPkgConfigDependency>,
) -> CargoResult<BTreeMap<String, PkgConfigLibrary>> {
    let mut results = BTreeMap::new();

    for (name, dep) in deps.iter() {
        let version_constraint = dep.version_constraint().unwrap_or("0");

        match query_pkg_config(name, version_constraint) {
            Ok(lib) => {
                results.insert(name.clone(), lib);
            }
            Err(e) => {
                // For now, we error on missing required dependencies
                // In Phase 2, we'll add support for optional=true and fallbacks
                return Err(e);
            }
        }
    }

    Ok(results)
}

/// Write the pkgconfig metadata file to OUT_DIR
pub fn write_metadata_file(
    out_dir: &Path,
    libraries: &BTreeMap<String, PkgConfigLibrary>,
) -> CargoResult<()> {
    let content = generate_metadata_file(libraries);
    let metadata_path = out_dir.join("pkgconfig_meta.rs");

    fs::write(&metadata_path, content)
        .with_context(|| format!("failed to write pkgconfig metadata to {}", metadata_path.display()))?;

    Ok(())
}

/// Probe and write pkgconfig metadata for all dependencies
///
/// This is the main entry point for integrating pkgconfig dependencies into the build.
/// It probes pkg-config for all declared dependencies and writes the metadata file.
pub fn probe_and_generate_metadata(
    deps: &BTreeMap<String, TomlPkgConfigDependency>,
    out_dir: &Path,
) -> CargoResult<()> {
    if deps.is_empty() {
        // No pkgconfig dependencies, write an empty module
        let empty_metadata = r#"// Auto-generated by Cargo from [pkgconfig-dependencies]
// This package has no pkgconfig-dependencies

#![allow(dead_code, non_upper_case_globals)]

/// Package metadata for pkgconfig-dependencies
pub mod pkgconfig {}
"#;
        let metadata_path = out_dir.join("pkgconfig_meta.rs");
        fs::write(&metadata_path, empty_metadata)
            .with_context(|| format!("failed to write pkgconfig metadata to {}", metadata_path.display()))?;
        return Ok(());
    }

    let libraries = probe_all_dependencies(deps)?;
    write_metadata_file(out_dir, &libraries)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_module_name() {
        assert_eq!(sanitize_module_name("libfoo"), "libfoo");
        assert_eq!(sanitize_module_name("lib-foo"), "lib_foo");
        assert_eq!(sanitize_module_name("lib.foo"), "lib_foo");
        assert_eq!(sanitize_module_name("gtk+-3.0"), "gtk__3_0");
        assert_eq!(sanitize_module_name("3dlib"), "lib_3dlib");
    }

    #[test]
    fn test_parse_pkg_config_output() {
        let cflags = "-I/usr/include/openssl -DOPENSSL_ENABLED";
        let ldflags = "-L/usr/lib -lssl -lcrypto";

        let (include_paths, lib_paths, libs, cflags_out, defines) =
            parse_pkg_config_output(cflags, ldflags);

        assert_eq!(include_paths, vec!["/usr/include/openssl"]);
        assert_eq!(lib_paths, vec!["/usr/lib"]);
        assert_eq!(libs, vec!["ssl", "crypto"]);
        assert_eq!(defines, vec!["OPENSSL_ENABLED"]);
        assert!(cflags_out.is_empty());
    }

    #[test]
    fn test_generate_metadata_file_empty() {
        let deps = BTreeMap::new();
        let content = generate_metadata_file(&deps);

        assert!(content.contains("pub mod pkgconfig"));
        assert!(content.contains("DO NOT EDIT"));
        assert!(content.contains("Auto-generated"));
    }

    #[test]
    fn test_generate_metadata_file_with_libs() {
        let mut deps = BTreeMap::new();
        deps.insert(
            "test-lib".to_string(),
            PkgConfigLibrary {
                name: "test-lib".to_string(),
                version: "1.0.0".to_string(),
                resolved_via: ResolutionMethod::PkgConfig,
                include_paths: vec!["/usr/include".to_string()],
                lib_paths: vec!["/usr/lib".to_string()],
                libs: vec!["testlib".to_string()],
                cflags: Vec::new(),
                defines: Vec::new(),
                ldflags: Vec::new(),
                raw_cflags: "-I/usr/include".to_string(),
                raw_ldflags: "-L/usr/lib -ltestlib".to_string(),
            },
        );

        let content = generate_metadata_file(&deps);

        assert!(content.contains("pub mod test_lib"));
        assert!(content.contains("VERSION"));
        assert!(content.contains("INCLUDE_PATHS"));
        assert!(content.contains("pub const VERSION: &str = \"1.0.0\""));
        assert!(content.contains("1.0.0"));
    }

    #[test]
    fn test_resolution_method_as_str() {
        assert_eq!(ResolutionMethod::PkgConfig.as_str(), "pkg-config");
        assert_eq!(ResolutionMethod::Fallback.as_str(), "fallback");
        assert_eq!(ResolutionMethod::NotFound.as_str(), "not-found");
        assert_eq!(ResolutionMethod::NotProbed.as_str(), "not-probed");
    }
}
