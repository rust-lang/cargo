use crate::core::compiler::CompileKind;
use crate::util::context::JobsConfig;
use crate::util::interning::InternedString;
use crate::util::{CargoResult, GlobalContext, RustfixDiagnosticServer};
use anyhow::{Context as _, bail};
use cargo_util::ProcessBuilder;
use serde::ser;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread::available_parallelism;

/// Configuration information for a rustc build.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// The requested kind of compilation for this session
    pub requested_kinds: Vec<CompileKind>,
    /// Number of rustc jobs to run in parallel.
    pub jobs: u32,
    /// Do not abort the build as soon as there is an error.
    pub keep_going: bool,
    /// Build profile
    pub requested_profile: InternedString,
    /// The intent we are compiling in.
    pub intent: UserIntent,
    /// `true` to print stdout in JSON format (for machine reading).
    pub message_format: MessageFormat,
    /// Force Cargo to do a full rebuild and treat each target as changed.
    pub force_rebuild: bool,
    /// Output the unit graph to stdout instead of actually compiling.
    pub unit_graph: bool,
    /// `true` to avoid really compiling.
    pub dry_run: bool,
    /// An optional override of the rustc process for primary units
    pub primary_unit_rustc: Option<ProcessBuilder>,
    /// A thread used by `cargo fix` to receive messages on a socket regarding
    /// the success/failure of applying fixes.
    pub rustfix_diagnostic_server: Rc<RefCell<Option<RustfixDiagnosticServer>>>,
    /// The directory to copy final artifacts to. Note that even if
    /// `artifact-dir` is set, a copy of artifacts still can be found at
    /// `target/(debug\release)` as usual.
    /// Named `export_dir` to avoid confusion with
    /// `CompilationFiles::artifact_dir`.
    pub export_dir: Option<PathBuf>,
    /// `true` to output a future incompatibility report at the end of the build
    pub future_incompat_report: bool,
    /// Which kinds of build timings to output (empty if none).
    pub timing_outputs: Vec<TimingOutput>,
    /// Output SBOM precursor files.
    pub sbom: bool,
    /// Build compile time dependencies only, e.g., build scripts and proc macros
    pub compile_time_deps_only: bool,
}

fn default_parallelism() -> CargoResult<u32> {
    Ok(available_parallelism()
        .context("failed to determine the amount of parallelism available")?
        .get() as u32)
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
        gctx: &GlobalContext,
        jobs: Option<JobsConfig>,
        keep_going: bool,
        requested_targets: &[String],
        intent: UserIntent,
    ) -> CargoResult<BuildConfig> {
        let cfg = gctx.build_config()?;
        let requested_kinds = CompileKind::from_requested_targets(gctx, requested_targets)?;
        if jobs.is_some() && gctx.jobserver_from_env().is_some() {
            gctx.shell().warn(
                "a `-j` argument was passed to Cargo but Cargo is \
                 also configured with an external jobserver in \
                 its environment, ignoring the `-j` parameter",
            )?;
        }
        let jobs = match jobs.or(cfg.jobs.clone()) {
            None => default_parallelism()?,
            Some(value) => match value {
                JobsConfig::Integer(j) => match j {
                    0 => anyhow::bail!("jobs may not be 0"),
                    j if j < 0 => (default_parallelism()? as i32 + j).max(1) as u32,
                    j => j as u32,
                },
                JobsConfig::String(j) => match j.as_str() {
                    "default" => default_parallelism()?,
                    _ => {
                        anyhow::bail!(format!(
                            "could not parse `{j}`. Number of parallel jobs should be `default` or a number."
                        ))
                    }
                },
            },
        };

        // If sbom flag is set, it requires the unstable feature
        let sbom = match (cfg.sbom, gctx.cli_unstable().sbom) {
            (Some(sbom), true) => sbom,
            (Some(_), false) => {
                gctx.shell()
                    .warn("ignoring 'sbom' config, pass `-Zsbom` to enable it")?;
                false
            }
            (None, _) => false,
        };

        Ok(BuildConfig {
            requested_kinds,
            jobs,
            keep_going,
            requested_profile: "dev".into(),
            intent,
            message_format: MessageFormat::Human,
            force_rebuild: false,
            unit_graph: false,
            dry_run: false,
            primary_unit_rustc: None,
            rustfix_diagnostic_server: Rc::new(RefCell::new(None)),
            export_dir: None,
            future_incompat_report: false,
            timing_outputs: Vec::new(),
            sbom,
            compile_time_deps_only: false,
        })
    }

    /// Whether or not the *user* wants JSON output. Whether or not rustc
    /// actually uses JSON is decided in `add_error_format`.
    pub fn emit_json(&self) -> bool {
        matches!(self.message_format, MessageFormat::Json { .. })
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

/// The specific action to be performed on each `Unit` of work.
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash, PartialOrd, Ord)]
pub enum CompileMode {
    /// Test with `rustc`.
    Test,
    /// Compile with `rustc`.
    Build,
    /// Type-check with `rustc` by emitting `rmeta` metadata only.
    ///
    /// If `test` is true, then it is also compiled with `--test` to check it like
    /// a test.
    Check { test: bool },
    /// Document with `rustdoc`.
    Doc,
    /// Test with `rustdoc`.
    Doctest,
    /// Scrape for function calls by `rustdoc`.
    Docscrape,
    /// Execute the binary built from the `build.rs` script.
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
            Doc { .. } => "doc".serialize(s),
            Doctest => "doctest".serialize(s),
            Docscrape => "docscrape".serialize(s),
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

    /// Returns `true` if this is scraping examples for documentation.
    pub fn is_doc_scrape(self) -> bool {
        self == CompileMode::Docscrape
    }

    /// Returns `true` if this is any type of test (test, benchmark, doc test, or
    /// check test).
    pub fn is_any_test(self) -> bool {
        matches!(
            self,
            CompileMode::Test | CompileMode::Check { test: true } | CompileMode::Doctest
        )
    }

    /// Returns `true` if this is something that passes `--test` to rustc.
    pub fn is_rustc_test(self) -> bool {
        matches!(self, CompileMode::Test | CompileMode::Check { test: true })
    }

    /// Returns `true` if this is the *execution* of a `build.rs` script.
    pub fn is_run_custom_build(self) -> bool {
        self == CompileMode::RunCustomBuild
    }

    /// Returns `true` if this mode may generate an executable.
    ///
    /// Note that this also returns `true` for building libraries, so you also
    /// have to check the target.
    pub fn generates_executable(self) -> bool {
        matches!(self, CompileMode::Test | CompileMode::Build)
    }
}

/// Represents the high-level operation requested by the user.
///
/// It determines which "Cargo targets" are selected by default and influences
/// how they will be processed. This is derived from the Cargo command the user
/// invoked (like `cargo build` or `cargo test`).
///
/// Unlike [`CompileMode`], which describes the specific compilation steps for
/// individual units, [`UserIntent`] represents the overall goal of the build
/// process as specified by the user.
///
/// For example, when a user runs `cargo test`, the intent is [`UserIntent::Test`],
/// but this might result in multiple [`CompileMode`]s for different units.
#[derive(Clone, Copy, Debug)]
pub enum UserIntent {
    /// Build benchmark binaries, e.g., `cargo bench`
    Bench,
    /// Build binaries and libraries, e.g., `cargo run`, `cargo install`, `cargo build`.
    Build,
    /// Perform type-check, e.g., `cargo check`.
    Check { test: bool },
    /// Document packages.
    ///
    /// If `deps` is true, then it will also document all dependencies.
    /// if `json` is true, the documentation output is in json format.
    Doc { deps: bool, json: bool },
    /// Build doctest binaries, e.g., `cargo test --doc`
    Doctest,
    /// Build test binaries, e.g., `cargo test`
    Test,
}

impl UserIntent {
    /// Returns `true` if this is generating documentation.
    pub fn is_doc(self) -> bool {
        matches!(self, UserIntent::Doc { .. })
    }

    /// User wants rustdoc output in JSON format.
    pub fn wants_doc_json_output(self) -> bool {
        matches!(self, UserIntent::Doc { json: true, .. })
    }

    /// User wants to document also for dependencies.
    pub fn wants_deps_docs(self) -> bool {
        matches!(self, UserIntent::Doc { deps: true, .. })
    }

    /// Returns `true` if this is any type of test (test, benchmark, doc test, or
    /// check test).
    pub fn is_any_test(self) -> bool {
        matches!(
            self,
            UserIntent::Test
                | UserIntent::Bench
                | UserIntent::Check { test: true }
                | UserIntent::Doctest
        )
    }

    /// Returns `true` if this is something that passes `--test` to rustc.
    pub fn is_rustc_test(self) -> bool {
        matches!(
            self,
            UserIntent::Test | UserIntent::Bench | UserIntent::Check { test: true }
        )
    }
}

/// Kinds of build timings we can output.
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash, PartialOrd, Ord)]
pub enum TimingOutput {
    /// Human-readable HTML report
    Html,
    /// Machine-readable JSON (unstable)
    Json,
}
