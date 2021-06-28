use crate::core::compiler::CompileKind;
use crate::util::interning::InternedString;
use crate::util::{CargoResult, Rustc, RustfixDiagnosticServer};
use crate::Config;
use anyhow::bail;
use cargo_util::ProcessBuilder;
use serde::ser;
use std::cell::RefCell;
use std::path::PathBuf;

/// Configuration information for a rustc build.
#[derive(Debug)]
pub struct BuildConfig {
    /// The requested kind of compilation for this session
    pub requested_kinds: Vec<CompileKind>,
    /// Number of rustc jobs to run in parallel.
    pub jobs: u32,
    /// Build profile
    pub requested_profile: InternedString,
    /// The mode we are compiling in.
    pub mode: CompileMode,
    /// `true` to print stdout in JSON format (for machine reading).
    pub message_format: MessageFormat,
    /// Force Cargo to do a full rebuild and treat each target as changed.
    pub force_rebuild: bool,
    /// Output a build plan to stdout instead of actually compiling.
    pub build_plan: bool,
    /// Output the unit graph to stdout instead of actually compiling.
    pub unit_graph: bool,
    /// An optional override of the rustc process for primary units
    pub primary_unit_rustc: Option<ProcessBuilder>,
    /// A thread used by `cargo fix` to receive messages on a socket regarding
    /// the success/failure of applying fixes.
    pub rustfix_diagnostic_server: RefCell<Option<RustfixDiagnosticServer>>,
    /// The directory to copy final artifacts to. Note that even if `out_dir` is
    /// set, a copy of artifacts still could be found a `target/(debug\release)`
    /// as usual.
    // Note that, although the cmd-line flag name is `out-dir`, in code we use
    // `export_dir`, to avoid confusion with out dir at `target/debug/deps`.
    pub export_dir: Option<PathBuf>,
    /// `true` to output a future incompatibility report at the end of the build
    pub future_incompat_report: bool,
}

impl BuildConfig {
    /// Parses all config files to learn about build configuration. Currently
    /// configured options are:
    ///
    /// * `build.jobs`
    /// * `build.target`
    /// * `target.$target.ar`
    /// * `target.$target.linker`
    /// * `target.$target.libfoo.metadata`
    pub fn new(
        config: &Config,
        rustc: CargoResult<Rustc>,
        jobs: Option<u32>,
        requested_targets: &[String],
        mode: CompileMode,
    ) -> CargoResult<BuildConfig> {
        let cfg = config.build_config()?;
        let requested_kinds =
            CompileKind::from_requested_targets(config, rustc, requested_targets)?;
        if jobs == Some(0) {
            anyhow::bail!("jobs must be at least 1")
        }
        if jobs.is_some() && config.jobserver_from_env().is_some() {
            config.shell().warn(
                "a `-j` argument was passed to Cargo but Cargo is \
                 also configured with an external jobserver in \
                 its environment, ignoring the `-j` parameter",
            )?;
        }
        let jobs = jobs.or(cfg.jobs).unwrap_or(::num_cpus::get() as u32);
        if jobs == 0 {
            anyhow::bail!("jobs may not be 0");
        }

        Ok(BuildConfig {
            requested_kinds,
            jobs,
            requested_profile: InternedString::new("dev"),
            mode,
            message_format: MessageFormat::Human,
            force_rebuild: false,
            build_plan: false,
            unit_graph: false,
            primary_unit_rustc: None,
            rustfix_diagnostic_server: RefCell::new(None),
            export_dir: None,
            future_incompat_report: false,
        })
    }

    /// Whether or not the *user* wants JSON output. Whether or not rustc
    /// actually uses JSON is decided in `add_error_format`.
    pub fn emit_json(&self) -> bool {
        matches!(self.message_format, MessageFormat::Json { .. })
    }

    pub fn test(&self) -> bool {
        self.mode == CompileMode::Test || self.mode == CompileMode::Bench
    }

    pub fn single_requested_kind(&self) -> CargoResult<CompileKind> {
        match self.requested_kinds.len() {
            1 => Ok(self.requested_kinds[0]),
            _ => bail!("only one `--target` argument is supported"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageFormat {
    Human,
    Json {
        /// Whether rustc diagnostics are rendered by cargo or included into the
        /// output stream.
        render_diagnostics: bool,
        /// Whether the `rendered` field of rustc diagnostics are using the
        /// "short" rendering.
        short: bool,
        /// Whether the `rendered` field of rustc diagnostics embed ansi color
        /// codes.
        ansi: bool,
    },
    Short,
}

/// The general "mode" for what to do.
/// This is used for two purposes. The commands themselves pass this in to
/// `compile_ws` to tell it the general execution strategy. This influences
/// the default targets selected. The other use is in the `Unit` struct
/// to indicate what is being done with a specific target.
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash, PartialOrd, Ord)]
pub enum CompileMode {
    /// A target being built for a test.
    Test,
    /// Building a target with `rustc` (lib or bin).
    Build,
    /// Building a target with `rustc` to emit `rmeta` metadata only. If
    /// `test` is true, then it is also compiled with `--test` to check it like
    /// a test.
    Check { test: bool },
    /// Used to indicate benchmarks should be built. This is not used in
    /// `Unit`, because it is essentially the same as `Test` (indicating
    /// `--test` should be passed to rustc) and by using `Test` instead it
    /// allows some de-duping of Units to occur.
    Bench,
    /// A target that will be documented with `rustdoc`.
    /// If `deps` is true, then it will also document all dependencies.
    Doc { deps: bool },
    /// A target that will be tested with `rustdoc`.
    Doctest,
    /// A marker for Units that represent the execution of a `build.rs` script.
    RunCustomBuild,
}

impl ser::Serialize for CompileMode {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use self::CompileMode::*;
        match *self {
            Test => "test".serialize(s),
            Build => "build".serialize(s),
            Check { .. } => "check".serialize(s),
            Bench => "bench".serialize(s),
            Doc { .. } => "doc".serialize(s),
            Doctest => "doctest".serialize(s),
            RunCustomBuild => "run-custom-build".serialize(s),
        }
    }
}

impl CompileMode {
    /// Returns `true` if the unit is being checked.
    pub fn is_check(self) -> bool {
        matches!(self, CompileMode::Check { .. })
    }

    /// Returns `true` if this is generating documentation.
    pub fn is_doc(self) -> bool {
        matches!(self, CompileMode::Doc { .. })
    }

    /// Returns `true` if this a doc test.
    pub fn is_doc_test(self) -> bool {
        self == CompileMode::Doctest
    }

    /// Returns `true` if this is any type of test (test, benchmark, doc test, or
    /// check test).
    pub fn is_any_test(self) -> bool {
        matches!(
            self,
            CompileMode::Test
                | CompileMode::Bench
                | CompileMode::Check { test: true }
                | CompileMode::Doctest
        )
    }

    /// Returns `true` if this is something that passes `--test` to rustc.
    pub fn is_rustc_test(self) -> bool {
        matches!(
            self,
            CompileMode::Test | CompileMode::Bench | CompileMode::Check { test: true }
        )
    }

    /// Returns `true` if this is the *execution* of a `build.rs` script.
    pub fn is_run_custom_build(self) -> bool {
        self == CompileMode::RunCustomBuild
    }
}
