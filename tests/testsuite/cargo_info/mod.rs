mod basic;
mod features;
mod features_activated_over_limit;
mod features_activated_over_limit_verbose;
mod features_deactivated_over_limit;
mod git_dependency;
mod help;
mod not_found;
mod path_dependency;
mod pick_msrv_compatible_package;
mod pick_msrv_compatible_package_within_ws;
mod pick_msrv_compatible_package_within_ws_and_use_msrv_from_ws;
mod specify_empty_version_with_url;
mod specify_version_outside_ws;
mod specify_version_with_url_but_registry_is_not_matched;
mod specify_version_within_ws_and_conflict_with_lockfile;
mod specify_version_within_ws_and_match_with_lockfile;
mod transitive_dependency_within_ws;
mod verbose;
mod with_frozen_outside_ws;
mod with_frozen_within_ws;
mod with_locked_outside_ws;
mod with_locked_within_ws;
mod with_locked_within_ws_and_pick_the_package;
mod with_offline;
mod with_quiet;
mod within_ws;
mod within_ws_and_pick_ws_package;
mod within_ws_with_alternative_registry;
mod within_ws_without_lockfile;
mod without_requiring_registry_auth;

// Initialize the registry without a token.
// Otherwise, it will try to list owners of the crate and fail.
pub(crate) fn init_registry_without_token() {
    let _reg = cargo_test_support::registry::RegistryBuilder::new()
        .no_configure_token()
        .build();
}
