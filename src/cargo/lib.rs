//! # Cargo as a library
//!
//! There are two places you can find API documentation of cargo-the-library,
//!
//! - <https://docs.rs/cargo>: targeted at external tool developers using cargo-the-library
//!   - Released with every rustc release
//! - <https://doc.rust-lang.org/nightly/nightly-rustc/cargo>: targeted at cargo contributors
//!   - Updated on each update of the `cargo` submodule in `rust-lang/rust`
//!
//! **WARNING:** Using Cargo as a library has drawbacks, particularly the API is unstable,
//! and there is no clear path to stabilize it soon at the time of writing.  See [The Cargo Book:
//! External tools] for more on this topic.
//!
//! ## Overview
//!
//! Major components of cargo include:
//!
//! - [`ops`]:
//!   Every major operation is implemented here. Each command is a thin wrapper around ops.
//!   - [`ops::cargo_compile`]:
//!     This is the entry point for all the compilation commands. This is a
//!     good place to start if you want to follow how compilation starts and
//!     flows to completion.
//! - [`ops::resolve`]:
//!   Top-level API for dependency and feature resolver (e.g. [`ops::resolve_ws`])
//!   - [`core::resolver`]: The core algorithm
//! - [`core::compiler`]:
//!   This is the code responsible for running `rustc` and `rustdoc`.
//!   - [`core::compiler::build_context`]:
//!     The [`BuildContext`][core::compiler::BuildContext] is the result of the "front end" of the
//!     build process. This contains the graph of work to perform and any settings necessary for
//!     `rustc`. After this is built, the next stage of building is handled in
//!     [`Context`][core::compiler::Context].
//!   - [`core::compiler::context`]:
//!     The `Context` is the mutable state used during the build process. This
//!     is the core of the build process, and everything is coordinated through
//!     this.
//!   - [`core::compiler::fingerprint`]:
//!     The `fingerprint` module contains all the code that handles detecting
//!     if a crate needs to be recompiled.
//! - [`sources::source`]:
//!   The [`sources::source::Source`] trait is an abstraction over different sources of packages.
//!   Sources are uniquely identified by a [`core::SourceId`]. Sources are implemented in the [`sources`]
//!   directory.
//! - [`util`]:
//!   This directory contains generally-useful utility modules.
//! - [`util::config`]:
//!   This directory contains the config parser. It makes heavy use of
//!   [serde](https://serde.rs/) to merge and translate config values. The
//!   [`util::Config`] is usually accessed from the
//!   [`core::Workspace`]
//!   though references to it are scattered around for more convenient access.
//! - [`util::toml`]:
//!   This directory contains the code for parsing `Cargo.toml` files.
//!   - [`ops::lockfile`]:
//!     This is where `Cargo.lock` files are loaded and saved.
//!
//! Related crates:
//! - [`cargo-platform`](https://crates.io/crates/cargo-platform)
//!   ([nightly docs](https://doc.rust-lang.org/nightly/nightly-rustc/cargo_platform)):
//!   This library handles parsing `cfg` expressions.
//! - [`cargo-util`](https://crates.io/crates/cargo-util)
//!   ([nightly docs](https://doc.rust-lang.org/nightly/nightly-rustc/cargo_util)):
//!   This contains general utility code that is shared between cargo and the testsuite
//! - [`crates-io`](https://crates.io/crates/crates-io)
//!   ([nightly docs](https://doc.rust-lang.org/nightly/nightly-rustc/crates_io)):
//!   This contains code for accessing the crates.io API.
//! - [`home`](https://crates.io/crates/home):
//!   This library is shared between cargo and rustup and is used for finding their home directories.
//!   This is not directly depended upon with a `path` dependency; cargo uses the version from crates.io.
//!   It is intended to be versioned and published independently of Rust's release system.
//!   Whenever a change needs to be made, bump the version in Cargo.toml and `cargo publish` it manually, and then update cargo's `Cargo.toml` to depend on the new version.
//! - [`cargo-test-support`](https://github.com/rust-lang/cargo/tree/master/crates/cargo-test-support)
//!   ([nightly docs](https://doc.rust-lang.org/nightly/nightly-rustc/cargo_test_support/index.html)):
//!   This contains a variety of code to support writing tests
//! - [`cargo-test-macro`](https://github.com/rust-lang/cargo/tree/master/crates/cargo-test-macro)
//!   ([nightly docs](https://doc.rust-lang.org/nightly/nightly-rustc/cargo_test_macro/index.html)):
//!   This is the `#[cargo_test]` proc-macro used by the test suite to define tests.
//! - [`credential`](https://github.com/rust-lang/cargo/tree/master/credential)
//!   This subdirectory contains several packages for implementing the
//!   [credential providers](https://doc.rust-lang.org/nightly/cargo/reference/registry-authentication.html).
//! - [`mdman`](https://github.com/rust-lang/cargo/tree/master/crates/mdman)
//!   ([nightly docs](https://doc.rust-lang.org/nightly/nightly-rustc/mdman/index.html)):
//!   This is a utility for generating cargo's man pages. See [Building the man
//!   pages](https://github.com/rust-lang/cargo/tree/master/src/doc#building-the-man-pages)
//!   for more information.
//! - [`resolver-tests`](https://github.com/rust-lang/cargo/tree/master/crates/resolver-tests)
//!   This is a dedicated package that defines tests for the [dependency
//!   resolver][core::resolver].
//!
//! ### File Overview
//!
//! Files that interact with cargo include
//!
//! - Package
//!   - `Cargo.toml`: User-written project manifest, loaded with [`util::toml::TomlManifest`] and then
//!     translated to [`core::manifest::Manifest`] which maybe stored in a [`core::Package`].
//!     - This is editable with [`util::toml_mut::manifest::LocalManifest`]
//!   - `Cargo.lock`: Generally loaded with [`ops::resolve_ws`] or a variant of it into a [`core::resolver::Resolve`]
//!     - At the lowest level, [`ops::load_pkg_lockfile`] and [`ops::write_pkg_lockfile`] are used
//!     - See [`core::resolver::encode`] for versioning of `Cargo.lock`
//!   - `target/`: Used for build artifacts and abstracted with [`core::compiler::layout`]. `Layout` handles locking the target directory and providing paths to parts inside. There is a separate `Layout` for each build `target`.
//!     - `target/debug/.fingerprint`: Tracker whether nor not a crate needs to be rebuilt.  See [`core::compiler::fingerprint`]
//! - `$CARGO_HOME/`:
//!   - `registry/`: Package registry cache which is managed in [`sources::registry`].  Be careful
//!     as the lock [`util::Config::acquire_package_cache_lock`] must be manually acquired.
//!     - `index`/: Fast-to-access crate metadata (no need to download / extract `*.crate` files)
//!     - `cache/*/*.crate`: Local cache of published crates
//!     - `src/*/*`: Extracted from `*.crate` by [`sources::registry::RegistrySource`]
//!   - `git/`: Git source cache.  See [`sources::git`].
//! - `**/.cargo/config.toml`: Environment dependent (env variables, files) configuration.  See
//!   [`util::config`]
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

// TODO: consider removing these lint attributes when `-Zlints` hits stable.
// For various reasons, some idioms are still allow'ed, but we would like to
// test and enforce them.
#![warn(rust_2018_idioms)]
// Due to some of the default clippy lints being somewhat subjective and not
// necessarily an improvement, we prefer to not use them at this time.
#![allow(clippy::all)]
#![warn(clippy::disallowed_methods)]
#![warn(clippy::self_named_module_files)]
#![warn(clippy::print_stdout)]
#![warn(clippy::print_stderr)]
#![warn(clippy::dbg_macro)]
#![allow(rustdoc::private_intra_doc_links)]

use crate::core::shell::Verbosity::Verbose;
use crate::core::Shell;
use anyhow::Error;
use tracing::debug;

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
