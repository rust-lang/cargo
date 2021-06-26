use crate::sources::CRATES_IO_DOMAIN;

pub use self::cargo_clean::{clean, CleanOptions};
pub use self::cargo_compile::{
    compile, compile_with_exec, compile_ws, create_bcx, print, resolve_all_features, CompileOptions,
};
pub use self::cargo_compile::{CompileFilter, FilterRule, LibRule, Packages};
pub use self::cargo_doc::{doc, DocOptions};
pub use self::cargo_fetch::{fetch, FetchOptions};
pub use self::cargo_generate_lockfile::generate_lockfile;
pub use self::cargo_generate_lockfile::update_lockfile;
pub use self::cargo_generate_lockfile::UpdateOptions;
pub use self::cargo_install::{install, install_list};
pub use self::cargo_new::{init, new, NewOptions, VersionControl};
pub use self::cargo_output_metadata::{output_metadata, ExportInfo, OutputMetadataOptions};
pub use self::cargo_package::{package, PackageOpts};
pub use self::cargo_pkgid::pkgid;
pub use self::cargo_read_manifest::{read_package, read_packages};
pub use self::cargo_run::run;
pub use self::cargo_test::{run_benches, run_tests, TestOptions};
pub use self::cargo_uninstall::uninstall;
pub use self::fix::{fix, fix_maybe_exec_rustc, FixOptions};
pub use self::lockfile::{load_pkg_lockfile, resolve_to_string, write_pkg_lockfile};
pub use self::registry::HttpTimeout;
pub use self::registry::{configure_http_handle, http_handle, http_handle_and_timeout};
pub use self::registry::{modify_owners, yank, OwnersOptions, PublishOpts};
pub use self::registry::{needs_custom_http_transport, registry_login, registry_logout, search};
pub use self::registry::{publish, registry_configuration, RegistryConfig};
pub use self::resolve::{
    add_overrides, get_resolved_packages, resolve_with_previous, resolve_ws, resolve_ws_with_opts,
};
pub use self::vendor::{vendor, VendorOptions};

mod cargo_clean;
mod cargo_compile;
pub mod cargo_config;
mod cargo_doc;
mod cargo_fetch;
mod cargo_generate_lockfile;
mod cargo_install;
mod cargo_new;
mod cargo_output_metadata;
mod cargo_package;
mod cargo_pkgid;
mod cargo_read_manifest;
mod cargo_run;
mod cargo_test;
mod cargo_uninstall;
mod common_for_install_and_uninstall;
mod fix;
mod lockfile;
mod registry;
mod resolve;
pub mod tree;
mod vendor;

/// Returns true if the dependency is either git or path, false otherwise
/// Error if a git/path dep is transitive, but has no version (registry source).
/// This check is performed on dependencies before publishing or packaging
fn check_dep_has_version(dep: &crate::core::Dependency, publish: bool) -> crate::CargoResult<bool> {
    let which = if dep.source_id().is_path() {
        "path"
    } else if dep.source_id().is_git() {
        "git"
    } else {
        return Ok(false);
    };

    if !dep.specified_req() && dep.is_transitive() {
        let dep_version_source = dep.registry_id().map_or_else(
            || CRATES_IO_DOMAIN.to_string(),
            |registry_id| registry_id.display_registry_name(),
        );
        anyhow::bail!(
            "all dependencies must have a version specified when {}.\n\
             dependency `{}` does not specify a version\n\
             Note: The {} dependency will use the version from {},\n\
             the `{}` specification will be removed from the dependency declaration.",
            if publish { "publishing" } else { "packaging" },
            dep.package_name(),
            if publish { "published" } else { "packaged" },
            dep_version_source,
            which,
        )
    }
    Ok(true)
}
