use crate::sources::CRATES_IO_DOMAIN;

pub use self::cargo_clean::{CleanContext, CleanOptions, clean};
pub use self::cargo_compile::unit_generator::UnitGenerator;
pub use self::cargo_compile::{CompileFilter, FilterRule, LibRule, Packages};
pub use self::cargo_compile::{
    CompileOptions, compile, compile_with_exec, compile_ws, create_bcx, print, resolve_all_features,
};
pub use self::cargo_doc::{DocOptions, OutputFormat, doc};
pub use self::cargo_fetch::{FetchOptions, fetch};
pub use self::cargo_install::{install, install_list};
pub use self::cargo_new::{NewOptions, NewProjectKind, VersionControl, init, new};
pub use self::cargo_output_metadata::{ExportInfo, OutputMetadataOptions, output_metadata};
pub use self::cargo_package::PackageMessageFormat;
pub use self::cargo_package::PackageOpts;
pub use self::cargo_package::check_yanked;
pub use self::cargo_package::package;
pub use self::cargo_pkgid::pkgid;
pub use self::cargo_read_manifest::read_package;
pub use self::cargo_run::run;
pub use self::cargo_test::{TestOptions, run_benches, run_tests};
pub use self::cargo_uninstall::uninstall;
pub use self::cargo_update::UpdateOptions;
pub use self::cargo_update::generate_lockfile;
pub use self::cargo_update::print_lockfile_changes;
pub use self::cargo_update::update_lockfile;
pub use self::cargo_update::upgrade_manifests;
pub use self::cargo_update::write_manifest_upgrades;
pub use self::common_for_install_and_uninstall::{InstallTracker, resolve_root};
pub use self::fix::{
    EditionFixMode, FixOptions, fix, fix_edition, fix_exec_rustc, fix_get_proxy_lock_addr,
};
pub use self::lockfile::{load_pkg_lockfile, resolve_to_string, write_pkg_lockfile};
pub use self::registry::OwnersOptions;
pub use self::registry::PublishOpts;
pub use self::registry::RegistryCredentialConfig;
pub use self::registry::RegistryOrIndex;
pub use self::registry::info;
pub use self::registry::modify_owners;
pub use self::registry::publish;
pub use self::registry::registry_login;
pub use self::registry::registry_logout;
pub use self::registry::search;
pub use self::registry::yank;
pub use self::resolve::{
    WorkspaceResolve, add_overrides, get_resolved_packages, resolve_with_previous, resolve_ws,
    resolve_ws_with_opts,
};
pub use self::vendor::{VendorOptions, vendor};

pub mod cargo_add;
mod cargo_clean;
pub(crate) mod cargo_compile;
pub mod cargo_config;
mod cargo_doc;
mod cargo_fetch;
mod cargo_install;
mod cargo_new;
mod cargo_output_metadata;
mod cargo_package;
mod cargo_pkgid;
mod cargo_read_manifest;
pub mod cargo_remove;
mod cargo_run;
mod cargo_test;
mod cargo_uninstall;
mod cargo_update;
mod common_for_install_and_uninstall;
mod fix;
pub(crate) mod lockfile;
pub(crate) mod registry;
pub(crate) mod resolve;
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
            "all dependencies must have a version requirement specified when {}.\n\
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
