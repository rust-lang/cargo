mod blanket_hint_mostly_unused;
mod im_a_teapot;
mod implicit_minimum_version_req;
mod missing_lints_inheritance;
mod non_kebab_case_bins;
mod non_kebab_case_features;
mod non_kebab_case_packages;
mod non_snake_case_features;
mod non_snake_case_packages;
mod redundant_homepage;
mod redundant_readme;
mod unknown_lints;
mod unused_dependencies;
mod unused_workspace_dependencies;
mod unused_workspace_package_fields;

pub use blanket_hint_mostly_unused::blanket_hint_mostly_unused;
pub use im_a_teapot::check_im_a_teapot;
pub use implicit_minimum_version_req::implicit_minimum_version_req_pkg;
pub use implicit_minimum_version_req::implicit_minimum_version_req_ws;
pub use missing_lints_inheritance::missing_lints_inheritance;
pub use non_kebab_case_bins::non_kebab_case_bins;
pub use non_kebab_case_features::non_kebab_case_features;
pub use non_kebab_case_packages::non_kebab_case_packages;
pub use non_snake_case_features::non_snake_case_features;
pub use non_snake_case_packages::non_snake_case_packages;
pub use redundant_homepage::redundant_homepage;
pub use redundant_readme::redundant_readme;
pub use unknown_lints::output_unknown_lints;
pub use unused_workspace_dependencies::unused_workspace_dependencies;
pub use unused_workspace_package_fields::unused_workspace_package_fields;

pub static LINTS: &[&crate::lints::Lint] = &[
    blanket_hint_mostly_unused::LINT,
    implicit_minimum_version_req::LINT,
    im_a_teapot::LINT,
    missing_lints_inheritance::LINT,
    non_kebab_case_bins::LINT,
    non_kebab_case_features::LINT,
    non_kebab_case_packages::LINT,
    non_snake_case_features::LINT,
    non_snake_case_packages::LINT,
    redundant_homepage::LINT,
    redundant_readme::LINT,
    unknown_lints::LINT,
    unused_dependencies::LINT,
    unused_workspace_dependencies::LINT,
    unused_workspace_package_fields::LINT,
];

/// Version required for specifying `[lints.cargo]`
///
/// Before this, it was an error.  No on-by-default lint should fire before this time without
/// another way of disabling it.
static CARGO_LINTS_MSRV: cargo_util_schemas::manifest::RustVersion =
    cargo_util_schemas::manifest::RustVersion::new(1, 79, 0);
