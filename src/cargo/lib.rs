// For various reasons, some idioms are still allow'ed, but we would like to
// test and enforce them.
#![warn(rust_2018_idioms)]
#![cfg_attr(test, deny(warnings))]
// Due to some of the default clippy lints being somewhat subjective and not
// necessarily an improvement, we prefer to not use them at this time.
#![allow(clippy::all)]

//! # Cargo as a library
//!
//! Cargo, the Rust package manager, is also provided as a library.
//!
//! There are two places you can find API documentation of cargo-the-library,
//!
//! - <https://docs.rs/cargo> and
//! - <https://doc.crates.io/contrib/apidoc/cargo>.
//!
//! Each of them targets on a slightly different audience.
//!
//! ## For external tool developers
//!
//! The documentation on <https://docs.rs/cargo> contains public-facing items in cargo-the-library.
//! External tool developers may find it useful when trying to reuse existing building blocks from Cargo.
//! However, using Cargo as a library has drawbacks, especially cargo-the-library is unstable,
//! and there is no clear path to stabilize it soon at the time of writing.
//! See [The Cargo Book: External tools] for more on this topic.
//!
//! Cargo API documentation on docs.rs gets updates along with each Rust release.
//! Its version always has a 0 major version to state it is unstable.
//! The minor version is always +1 of rustc's minor version
//! (that is, `cargo 0.66.0` corresponds to `rustc 1.65`).
//!
//! ## For Cargo contributors
//!
//! The documentation on <https://doc.crates.io/contrib/apidoc/cargo> contains all items in Cargo.
//! Contributors of Cargo may find it useful as a reference of Cargo's implementation details.
//! It's built with `--document-private-items` rustdoc flag,
//! so you might expect to see some noise and strange items here.
//! The Cargo team and contributors strive for jotting down every details
//! from their brains in each issue and PR.
//! However, something might just disappear in the air with no reason.
//! This documentation can be seen as their extended minds,
//! sharing designs and hacks behind both public and private interfaces.
//!
//! If you are just diving into Cargo internals, [Cargo Architecture Overview]
//! is the best material to get a broader context of how Cargo works under the hood.
//! Things also worth a read are important concepts reside in source code,
//! which Cargo developers have been crafting for a while, namely
//!
//! - [`cargo::core::resolver`](crate::core::resolver),
//! - [`cargo::core::compiler::fingerprint`](core/compiler/fingerprint/index.html),
//! - [`cargo::util::config`](crate::util::config),
//! - [`cargo::ops::fix`](ops/fix/index.html), and
//! - [`cargo::sources::registry`](crate::sources::registry).
//!
//! This API documentation is published on each push of rust-lang/cargo master branch.
//! In other words, it always reflects the latest doc comments in source code on master branch.
//!
//! ## Contribute to Cargo documentations
//!
//! The Cargo team always continues improving all external and internal documentations.
//! If you spot anything could be better, don't hesitate to discuss with the team on
//! Zulip [`t-cargo` stream], or [submit an issue] right on GitHub.
//! There is also an issue label [`A-documenting-cargo-itself`],
//! which is generally for documenting user-facing [The Cargo Book],
//! but the Cargo team is welcome any form of enhancement for the [Cargo Contributor Guide]
//! and this API documentation as well.
//!
//! [The Cargo Book: External tools]: https://doc.rust-lang.org/stable/cargo/reference/external-tools.html
//! [Cargo Architecture Overview]: https://doc.crates.io/contrib/architecture
//! [`t-cargo` stream]: https://rust-lang.zulipchat.com/#narrow/stream/246057-t-cargo
//! [submit an issue]: https://github.com/rust-lang/cargo/issues/new/choose
//! [`A-documenting-cargo-itself`]: https://github.com/rust-lang/cargo/labels/A-documenting-cargo-itself
//! [The Cargo Book]: https://doc.rust-lang.org/cargo/
//! [Cargo Contributor Guide]: https://doc.crates.io/contrib/

use crate::core::shell::Verbosity::Verbose;
use crate::core::Shell;
use anyhow::Error;
use log::debug;

pub use crate::util::errors::{AlreadyPrintedError, InternalError, VerboseError};
pub use crate::util::{indented_lines, CargoResult, CliError, CliResult, Config};
pub use crate::version::version;

pub const CARGO_ENV: &str = "CARGO";

#[macro_use]
mod macros;

pub mod core;
pub mod ops;
pub mod sources;
pub mod util;
mod version;

pub fn exit_with_error(err: CliError, shell: &mut Shell) -> ! {
    debug!("exit_with_error; err={:?}", err);

    if let Some(ref err) = err.error {
        if let Some(clap_err) = err.downcast_ref::<clap::Error>() {
            let exit_code = if clap_err.use_stderr() { 1 } else { 0 };
            let _ = clap_err.print();
            std::process::exit(exit_code)
        }
    }

    let CliError { error, exit_code } = err;
    if let Some(error) = error {
        display_error(&error, shell);
    }

    std::process::exit(exit_code)
}

/// Displays an error, and all its causes, to stderr.
pub fn display_error(err: &Error, shell: &mut Shell) {
    debug!("display_error; err={:?}", err);
    _display_error(err, shell, true);
    if err
        .chain()
        .any(|e| e.downcast_ref::<InternalError>().is_some())
    {
        drop(shell.note("this is an unexpected cargo internal error"));
        drop(
            shell.note(
                "we would appreciate a bug report: https://github.com/rust-lang/cargo/issues/",
            ),
        );
        drop(shell.note(format!("cargo {}", version())));
        // Once backtraces are stabilized, this should print out a backtrace
        // if it is available.
    }
}

/// Displays a warning, with an error object providing detailed information
/// and context.
pub fn display_warning_with_error(warning: &str, err: &Error, shell: &mut Shell) {
    drop(shell.warn(warning));
    drop(writeln!(shell.err()));
    _display_error(err, shell, false);
}

fn _display_error(err: &Error, shell: &mut Shell, as_err: bool) -> bool {
    for (i, err) in err.chain().enumerate() {
        // If we're not in verbose mode then only print cause chain until one
        // marked as `VerboseError` appears.
        //
        // Generally the top error shouldn't be verbose, but check it anyways.
        if shell.verbosity() != Verbose && err.is::<VerboseError>() {
            return true;
        }
        if err.is::<AlreadyPrintedError>() {
            break;
        }
        if i == 0 {
            if as_err {
                drop(shell.error(&err));
            } else {
                drop(writeln!(shell.err(), "{}", err));
            }
        } else {
            drop(writeln!(shell.err(), "\nCaused by:"));
            drop(write!(shell.err(), "{}", indented_lines(&err.to_string())));
        }
    }
    false
}
