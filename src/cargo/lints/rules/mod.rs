mod blanket_hint_mostly_unused;
mod im_a_teapot;
mod implicit_minimum_version_req;
mod non_kebab_case_bin;
mod text_direction_codepoint;
mod unknown_lints;

pub use blanket_hint_mostly_unused::blanket_hint_mostly_unused;
pub use im_a_teapot::check_im_a_teapot;
pub use implicit_minimum_version_req::implicit_minimum_version_req;
pub use non_kebab_case_bin::non_kebab_case_bin;
pub use text_direction_codepoint::text_direction_codepoint;
pub use unknown_lints::output_unknown_lints;

pub const LINTS: &[crate::lints::Lint] = &[
    blanket_hint_mostly_unused::LINT,
    implicit_minimum_version_req::LINT,
    im_a_teapot::LINT,
    non_kebab_case_bin::LINT,
    text_direction_codepoint::LINT,
    unknown_lints::LINT,
];
