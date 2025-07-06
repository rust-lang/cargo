use std::path::PathBuf;

use cargo_test_support::{ArgLineCommandExt, Execs, Project, TestEnvCommandExt, compare};

pub trait CargoProjectExt {
    /// Creates a `ProcessBuilder` to run cargo.
    ///
    /// Arguments can be separated by spaces.
    ///
    /// For `cargo run`, see [`Project::rename_run`].
    ///
    /// # Example:
    ///
    /// ```no_run
    /// # let p = cargo_test_support::project().build();
    /// p.cargo("build --bin foo").run();
    /// ```
    fn cargo(&self, cmd: &str) -> Execs;
}

impl CargoProjectExt for Project {
    fn cargo(&self, cmd: &str) -> Execs {
        let cargo = cargo_exe();
        let mut execs = self.process(&cargo);
        execs.env("CARGO", cargo);
        execs.arg_line(cmd);
        execs
    }
}

/// Path to the cargo binary
pub fn cargo_exe() -> PathBuf {
    snapbox::cmd::cargo_bin!("cargo").to_path_buf()
}

/// Test the cargo command
pub trait CargoCommandExt {
    fn cargo_ui() -> Self;
}

impl CargoCommandExt for snapbox::cmd::Command {
    fn cargo_ui() -> Self {
        Self::new(cargo_exe())
            .with_assert(compare::assert_ui())
            .env("CARGO_TERM_COLOR", "always")
            .env("CARGO_TERM_HYPERLINKS", "true")
            .test_env()
    }
}
