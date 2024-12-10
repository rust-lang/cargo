//! build-rs provides a strongly typed interface around the Cargo build script
//! protocol. Cargo provides inputs to the build script by environment variable
//! and accepts commands by printing to stdout.
//!
//! > This crate is maintained by the Cargo team for use by the wider
//! > ecosystem. This crate follows semver compatibility for its APIs.
#![cfg_attr(all(doc, feature = "unstable"), feature(doc_auto_cfg, doc_cfg))]
#![allow(clippy::disallowed_methods)] // HACK: deferred resoling this
#![allow(clippy::print_stdout)] // HACK: deferred resoling this

#[cfg(feature = "unstable")]
macro_rules! unstable {
    ($feature:ident, $issue:literal) => {
        concat!(
            r#"<div class="stab unstable">"#,
            r#"<span class="emoji">🔬</span>"#,
            r#"<span>This is a nightly-only experimental API. (<code>"#,
            stringify!($feature),
            r#"</code>&nbsp;<a href="https://github.com/rust-lang/rust/issues/"#,
            $issue,
            r#"">#"#,
            $issue,
            r#"</a>)</span>"#,
            r#"</div>"#
        )
    };
}

macro_rules! respected_msrv {
    ($ver:literal) => {
        concat!(
            r#"<div class="warning">

MSRV: Respected as of "#,
            $ver,
            r#".

</div>"#
        )
    };
}

macro_rules! requires_msrv {
    ($ver:literal) => {
        concat!(
            r#"<div class="warning">

MSRV: Requires "#,
            $ver,
            r#".

</div>"#
        )
    };
}

mod ident;

pub mod input;
pub mod output;
