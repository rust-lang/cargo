//! Support for pkg-config dependencies declared in Cargo.toml
//!
//! This module handles querying system pkg-config for dependencies declared
//! in the `[pkgconfig-dependencies]` section and generating metadata.
//!
//! # Usage
//!
//! Declare pkgconfig dependencies in Cargo.toml under an unstable feature flag:
//!
//! ```toml
//! # Requires: cargo build -Z pkgconfig-dependencies
//!
//! [pkgconfig-dependencies]
//! # Simple form: just version constraint
//! openssl = "1.1"
//!
//! # Detailed form with additional options
//! [pkgconfig-dependencies.sqlite3]
//! version = "3.0"
//! # Try alternative pkg-config names
//! names = ["sqlite3", "sqlite"]
//! # Mark as optional - doesn't fail build if not found
//! optional = true
//! # Fallback specification if pkg-config fails
//! [pkgconfig-dependencies.sqlite3.fallback]
//! libs = ["sqlite3"]
//! lib-paths = ["/usr/local/lib"]
//! include-paths = ["/usr/local/include"]
//! ```
//!
//! # Generated Metadata
//!
//! The module generates `OUT_DIR/pkgconfig_meta.rs` with compile-time constants:
//!
//! ```ignore
//! pub mod pkgconfig {
//!     pub mod openssl {
//!         pub const VERSION: &str = "1.1.1";
//!         pub const FOUND: bool = true;
//!         pub const RESOLVED_VIA: &str = "pkg-config";
//!         pub const INCLUDE_PATHS: &[&str] = &["/usr/include"];
//!         pub const LIB_PATHS: &[&str] = &["/usr/lib"];
//!         pub const LIBS: &[&str] = &["ssl", "crypto"];
//!         // ... and more fields
//!     }
//! }
//! ```
//!
//! # Accessing Metadata
//!
//! In your build script or build-time code:
//!
//! ```ignore
//! include!(concat!(env!("OUT_DIR"), "/pkgconfig_meta.rs"));
//!
//! fn main() {
//!     let version = pkgconfig::openssl::VERSION;
//!     let is_found = pkgconfig::openssl::FOUND;
//!
//!     if is_found {
//!         for lib in pkgconfig::openssl::LIBS {
//!             println!("cargo:rustc-link-lib={}", lib);
//!         }
//!     }
//! }
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context as _};
use cargo_util_schemas::manifest::{TomlPkgConfigDependency, TomlPkgConfigFallback};

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

    // Format arrays for Rust code (helper for potential future use)
    let _format_str_array = |items: &[String]| -> String {
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
///
/// Creates a Rust module with compile-time constants for each dependency.
///
/// # Output Structure
///
/// For each dependency, creates a module like:
/// ```ignore
/// pub mod pkgconfig {
///     pub mod dependency_name {
///         pub const VERSION: &str = "1.0.0";
///         pub const FOUND: bool = true;
///         pub const RESOLVED_VIA: &str = "pkg-config";
///         pub const INCLUDE_PATHS: &[&str] = &["/usr/include", ...];
///         pub const LIB_PATHS: &[&str] = &["/usr/lib", ...];
///         pub const LIBS: &[&str] = &["lib1", "lib2"];
///         pub const CFLAGS: &[&str] = &[...];
///         pub const DEFINES: &[&str] = &[...];
///         pub const LDFLAGS: &[&str] = &[...];
///         pub const RAW_CFLAGS: &str = "-I/usr/include ...";
///         pub const RAW_LDFLAGS: &str = "-L/usr/lib -llib1 -llib2";
///     }
/// }
/// ```
///
/// Module names are sanitized for Rust identifier rules (special chars become underscores).
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

/// Apply fallback specification as a PkgConfigLibrary
fn apply_fallback(
    name: &str,
    fallback: &TomlPkgConfigFallback,
) -> PkgConfigLibrary {
    let libs = fallback.libs.as_ref().map(|v| v.clone()).unwrap_or_default();
    let include_paths = fallback
        .include_paths
        .as_ref()
        .map(|v| v.clone())
        .unwrap_or_default();
    let lib_paths = fallback
        .lib_paths
        .as_ref()
        .map(|v| v.clone())
        .unwrap_or_default();

    // Reconstruct raw flags from fallback
    let raw_cflags = {
        let mut parts = Vec::new();
        for path in &include_paths {
            parts.push(format!("-I{}", path));
        }
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

    PkgConfigLibrary {
        name: name.to_string(),
        version: String::new(),
        resolved_via: ResolutionMethod::Fallback,
        include_paths,
        lib_paths,
        libs,
        cflags: Vec::new(),
        defines: Vec::new(),
        ldflags: Vec::new(),
        raw_cflags,
        raw_ldflags,
    }
}

/// Query pkg-config for a single dependency by name
fn query_pkg_config_by_name(
    name: &str,
    version_constraint: &str,
) -> CargoResult<pkg_config::Library> {
    let mut config = pkg_config::Config::new();
    config.atleast_version(version_constraint);
    config.probe(name).map_err(|e| anyhow::anyhow!("{}", e))
}

/// Query pkg-config for a single dependency
///
/// # Resolution Strategy
///
/// 1. First tries the primary package name
/// 2. If that fails, tries each alternative name in order (if provided)
/// 3. If all pkg-config queries fail and a fallback is provided, uses the fallback spec
/// 4. Returns the first successful match
///
/// # Arguments
///
/// * `name` - Primary package name for this dependency
/// * `version_constraint` - Minimum version constraint (passed to pkg-config)
/// * `alternative_names` - Optional list of alternative pkg-config names to try
/// * `fallback` - Optional fallback specification for manual configuration
///
/// # Returns
///
/// Returns `PkgConfigLibrary` with resolved metadata if found via pkg-config or fallback.
/// Returns `Err` with detailed error message if all resolution methods fail.
///
/// # Errors
///
/// Returns error if:
/// - All pkg-config names fail AND no fallback is provided
/// - The fallback spec itself is invalid
pub fn query_pkg_config(
    name: &str,
    version_constraint: &str,
    alternative_names: Option<&[String]>,
    fallback: Option<&TomlPkgConfigFallback>,
) -> CargoResult<PkgConfigLibrary> {
    // Collect all names to try, with the primary name first
    let mut names_to_try = vec![name.to_string()];
    if let Some(alts) = alternative_names {
        names_to_try.extend(alts.iter().cloned());
    }

    // Try each name in order
    let mut last_error = None;
    for try_name in names_to_try.iter() {
        match query_pkg_config_by_name(try_name, version_constraint) {
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

                return Ok(PkgConfigLibrary {
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
                });
            }
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        }
    }

    // All names failed - try fallback if available
    if let Some(fb) = fallback {
        return Ok(apply_fallback(name, fb));
    }

    // No fallback available - report error with helpful suggestions
    let mut error_msg = format!(
        "pkg-config dependency `{}` (version {}) not found",
        name, version_constraint
    );

    if names_to_try.len() > 1 {
        error_msg.push_str(&format!("\n  tried pkg-config names: {}", names_to_try.join(", ")));
    }

    if let Some(e) = last_error {
        error_msg.push_str(&format!("\n  error: {}", e));
    }

    error_msg.push_str(&format!(
        "\n\nTo fix this, you can:\n\
         1. Install the system library (e.g., libfoo-dev on Debian, libfoo-devel on Fedora)\n\
         2. Set the PKG_CONFIG_PATH environment variable to include the directory with the .pc file\n\
         3. Add a [fallback] specification in Cargo.toml to manually specify library paths\n\
         4. Use alternative names via the `names` field if the package has multiple names"
    ));

    bail!("{}", error_msg)
}

/// Probe all pkgconfig dependencies for a package
pub fn probe_all_dependencies(
    deps: &BTreeMap<String, TomlPkgConfigDependency>,
) -> CargoResult<BTreeMap<String, PkgConfigLibrary>> {
    let mut results = BTreeMap::new();

    for (name, dep) in deps.iter() {
        let version_constraint = dep.version_constraint().unwrap_or("0");
        let alternative_names = dep.names();
        let is_optional = dep.is_optional();
        let fallback = dep.fallback();

        match query_pkg_config(name, version_constraint, alternative_names, fallback) {
            Ok(lib) => {
                results.insert(name.clone(), lib);
            }
            Err(e) => {
                if is_optional {
                    // For optional dependencies, insert a "not found" entry
                    // Log a warning so users know which optional dependency wasn't found
                    eprintln!(
                        "warning: optional pkg-config dependency `{}` not found (ignoring)\n  {}",
                        name, e
                    );

                    results.insert(
                        name.clone(),
                        PkgConfigLibrary {
                            name: name.clone(),
                            version: String::new(),
                            resolved_via: ResolutionMethod::NotFound,
                            include_paths: Vec::new(),
                            lib_paths: Vec::new(),
                            libs: Vec::new(),
                            cflags: Vec::new(),
                            defines: Vec::new(),
                            ldflags: Vec::new(),
                            raw_cflags: String::new(),
                            raw_ldflags: String::new(),
                        },
                    );
                } else {
                    // Required dependency not found - error
                    return Err(e);
                }
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
///
/// # Process
///
/// 1. Probes each declared dependency using pkg-config
/// 2. For each dependency, tries alternative names if specified
/// 3. Uses fallback specification if pkg-config fails
/// 4. Handles optional dependencies (doesn't fail build if not found)
/// 5. Generates Rust code with compile-time constants
/// 6. Writes `OUT_DIR/pkgconfig_meta.rs` for inclusion in build scripts
///
/// # Returns
///
/// Returns `Ok(())` if all required dependencies are found.
/// Returns `Err` if any required (non-optional) dependency is not found.
///
/// # Panics
///
/// Does not panic. All errors are returned as `CargoResult`.
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

    #[test]
    fn test_sanitize_module_name_edge_cases() {
        // Test with special characters
        assert_eq!(sanitize_module_name("lib++"), "lib__");
        assert_eq!(sanitize_module_name("lib-3.0"), "lib_3_0");
        assert_eq!(sanitize_module_name("PKG_CONFIG"), "pkg_config");

        // Test starting with digit
        assert_eq!(sanitize_module_name("123"), "lib_123");
        assert_eq!(sanitize_module_name("2to3"), "lib_2to3");
    }

    #[test]
    fn test_generate_metadata_file_module_naming() {
        let mut deps = BTreeMap::new();

        // Test various names that need sanitization
        deps.insert(
            "gtk+-3.0".to_string(),
            PkgConfigLibrary {
                name: "gtk+-3.0".to_string(),
                version: "3.0".to_string(),
                resolved_via: ResolutionMethod::PkgConfig,
                include_paths: vec![],
                lib_paths: vec![],
                libs: vec![],
                cflags: vec![],
                defines: vec![],
                ldflags: vec![],
                raw_cflags: String::new(),
                raw_ldflags: String::new(),
            },
        );

        let content = generate_metadata_file(&deps);

        // Should sanitize the module name
        assert!(content.contains("pub mod gtk_plus_3_0"));
        // But keep the original name in comments
        assert!(content.contains("gtk+-3.0"));
    }

    #[test]
    fn test_pkgconfig_library_with_multiple_items() {
        let mut libs = BTreeMap::new();

        libs.insert(
            "lib1".to_string(),
            PkgConfigLibrary {
                name: "lib1".to_string(),
                version: "1.0".to_string(),
                resolved_via: ResolutionMethod::PkgConfig,
                include_paths: vec!["/usr/include/lib1".to_string()],
                lib_paths: vec!["/usr/lib".to_string()],
                libs: vec!["lib1".to_string()],
                cflags: vec![],
                defines: vec!["LIB1_ENABLED".to_string()],
                ldflags: vec![],
                raw_cflags: "-I/usr/include/lib1 -DLIB1_ENABLED".to_string(),
                raw_ldflags: "-L/usr/lib -llib1".to_string(),
            },
        );

        libs.insert(
            "lib2".to_string(),
            PkgConfigLibrary {
                name: "lib2".to_string(),
                version: "2.0".to_string(),
                resolved_via: ResolutionMethod::PkgConfig,
                include_paths: vec!["/usr/include/lib2".to_string()],
                lib_paths: vec!["/usr/lib".to_string()],
                libs: vec!["lib2".to_string()],
                cflags: vec![],
                defines: vec![],
                ldflags: vec![],
                raw_cflags: "-I/usr/include/lib2".to_string(),
                raw_ldflags: "-L/usr/lib -llib2".to_string(),
            },
        );

        let content = generate_metadata_file(&libs);

        // Should have both modules
        assert!(content.contains("pub mod lib1"));
        assert!(content.contains("pub mod lib2"));
        // Each should have their own version
        assert!(content.contains("pub const VERSION: &str = \"1.0\""));
        assert!(content.contains("pub const VERSION: &str = \"2.0\""));
    }

    #[test]
    fn test_apply_fallback_creates_library() {
        use cargo_util_schemas::manifest::TomlPkgConfigFallback;

        let fallback = TomlPkgConfigFallback {
            libs: Some(vec!["mylib".to_string()]),
            lib_paths: Some(vec!["/usr/local/lib".to_string()]),
            include_paths: Some(vec!["/usr/local/include".to_string()]),
        };

        let lib = apply_fallback("mylib", &fallback);

        assert_eq!(lib.name, "mylib");
        assert_eq!(lib.libs, vec!["mylib"]);
        assert_eq!(lib.lib_paths, vec!["/usr/local/lib"]);
        assert_eq!(lib.include_paths, vec!["/usr/local/include"]);
        assert_eq!(lib.resolved_via.as_str(), "fallback");
        assert!(lib.version.is_empty());
    }

    #[test]
    fn test_apply_fallback_empty_values() {
        use cargo_util_schemas::manifest::TomlPkgConfigFallback;

        let fallback = TomlPkgConfigFallback {
            libs: None,
            lib_paths: None,
            include_paths: None,
        };

        let lib = apply_fallback("test", &fallback);

        assert_eq!(lib.name, "test");
        assert!(lib.libs.is_empty());
        assert!(lib.lib_paths.is_empty());
        assert!(lib.include_paths.is_empty());
        assert_eq!(lib.resolved_via.as_str(), "fallback");
    }

    #[test]
    fn test_generate_metadata_with_fallback_resolution() {
        let mut deps = BTreeMap::new();
        deps.insert(
            "fallback-lib".to_string(),
            PkgConfigLibrary {
                name: "fallback-lib".to_string(),
                version: String::new(),
                resolved_via: ResolutionMethod::Fallback,
                include_paths: vec!["/custom/include".to_string()],
                lib_paths: vec!["/custom/lib".to_string()],
                libs: vec!["customlib".to_string()],
                cflags: Vec::new(),
                defines: Vec::new(),
                ldflags: Vec::new(),
                raw_cflags: "-I/custom/include".to_string(),
                raw_ldflags: "-L/custom/lib -lcustomlib".to_string(),
            },
        );

        let content = generate_metadata_file(&deps);

        assert!(content.contains("pub mod fallback_lib"));
        assert!(content.contains("\"fallback\""));
        assert!(content.contains("pub const RESOLVED_VIA: &str = \"fallback\""));
        assert!(content.contains("pub const FOUND: bool = true"));
    }

    #[test]
    fn test_generate_metadata_with_not_found_resolution() {
        let mut deps = BTreeMap::new();
        deps.insert(
            "missing-lib".to_string(),
            PkgConfigLibrary {
                name: "missing-lib".to_string(),
                version: String::new(),
                resolved_via: ResolutionMethod::NotFound,
                include_paths: Vec::new(),
                lib_paths: Vec::new(),
                libs: Vec::new(),
                cflags: Vec::new(),
                defines: Vec::new(),
                ldflags: Vec::new(),
                raw_cflags: String::new(),
                raw_ldflags: String::new(),
            },
        );

        let content = generate_metadata_file(&deps);

        assert!(content.contains("pub mod missing_lib"));
        assert!(content.contains("pub const FOUND: bool = false"));
        assert!(content.contains("\"not-found\""));
    }
}
