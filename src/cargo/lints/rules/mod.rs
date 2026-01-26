mod blanket_hint_mostly_unused;
mod im_a_teapot;
mod implicit_minimum_version_req;
mod non_kebab_case_bins;
mod non_kebab_case_packages;
mod non_snake_case_packages;
mod redundant_readme;
mod unknown_lints;

pub use blanket_hint_mostly_unused::blanket_hint_mostly_unused;
pub use im_a_teapot::check_im_a_teapot;
pub use implicit_minimum_version_req::implicit_minimum_version_req;
pub use non_kebab_case_bins::non_kebab_case_bins;
pub use non_kebab_case_packages::non_kebab_case_packages;
pub use non_snake_case_packages::non_snake_case_packages;
pub use redundant_readme::redundant_readme;
pub use unknown_lints::output_unknown_lints;

pub const LINTS: &[crate::lints::Lint] = &[
    blanket_hint_mostly_unused::LINT,
    implicit_minimum_version_req::LINT,
    im_a_teapot::LINT,
    non_kebab_case_bins::LINT,
    non_kebab_case_packages::LINT,
    non_snake_case_packages::LINT,
    redundant_readme::LINT,
    unknown_lints::LINT,
];
