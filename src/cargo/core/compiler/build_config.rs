use std::collections::HashMap;
use std::path::{Path, PathBuf};
use util::{CargoResult, CargoResultExt, Config, Rustc};
use super::BuildOutput;

/// Configuration information for a rustc build.
pub struct BuildConfig {
    pub rustc: Rustc,
    /// Build information for the host arch
    pub host: TargetConfig,
    /// The target arch triple, defaults to host arch
    pub requested_target: Option<String>,
    /// Build information for the target
    pub target: TargetConfig,
    /// How many rustc jobs to run in parallel
    pub jobs: u32,
    /// Whether we are building for release
    pub release: bool,
    /// In what mode we are compiling
    pub mode: CompileMode,
    /// Whether to print std output in json format (for machine reading)
    pub message_format: MessageFormat,
}

impl BuildConfig {
    /// Parse all config files to learn about build configuration. Currently
    /// configured options are:
    ///
    /// * build.jobs
    /// * build.target
    /// * target.$target.ar
    /// * target.$target.linker
    /// * target.$target.libfoo.metadata
    pub fn new(
        config: &Config,
        jobs: Option<u32>,
        requested_target: &Option<String>,
        rustc_info_cache: Option<PathBuf>,
        mode: CompileMode,
    ) -> CargoResult<BuildConfig> {
        let requested_target = match requested_target {
            &Some(ref target) if target.ends_with(".json") => {
                let path = Path::new(target)
                    .canonicalize()
                    .chain_err(|| format_err!("Target path {:?} is not a valid file", target))?;
                Some(path.into_os_string()
                    .into_string()
                    .map_err(|_| format_err!("Target path is not valid unicode"))?)
            }
            other => other.clone(),
        };
        if let Some(ref s) = requested_target {
            if s.trim().is_empty() {
                bail!("target was empty")
            }
        }
        let cfg_target = config.get_string("build.target")?.map(|s| s.val);
        let target = requested_target.clone().or(cfg_target);

        if jobs == Some(0) {
            bail!("jobs must be at least 1")
        }
        if jobs.is_some() && config.jobserver_from_env().is_some() {
            config.shell().warn(
                "a `-j` argument was passed to Cargo but Cargo is \
                 also configured with an external jobserver in \
                 its environment, ignoring the `-j` parameter",
            )?;
        }
        let cfg_jobs = match config.get_i64("build.jobs")? {
            Some(v) => {
                if v.val <= 0 {
                    bail!(
                        "build.jobs must be positive, but found {} in {}",
                        v.val,
                        v.definition
                    )
                } else if v.val >= i64::from(u32::max_value()) {
                    bail!(
                        "build.jobs is too large: found {} in {}",
                        v.val,
                        v.definition
                    )
                } else {
                    Some(v.val as u32)
                }
            }
            None => None,
        };
        let jobs = jobs.or(cfg_jobs).unwrap_or(::num_cpus::get() as u32);
        let rustc = config.rustc(rustc_info_cache)?;
        let host_config = TargetConfig::new(config, &rustc.host)?;
        let target_config = match target.as_ref() {
            Some(triple) => TargetConfig::new(config, triple)?,
            None => host_config.clone(),
        };
        Ok(BuildConfig {
            rustc,
            requested_target: target,
            jobs,
            host: host_config,
            target: target_config,
            release: false,
            mode,
            message_format: MessageFormat::Human,
        })
    }

    /// The host arch triple
    ///
    /// e.g. x86_64-unknown-linux-gnu, would be
    ///  - machine: x86_64
    ///  - hardware-platform: unknown
    ///  - operating system: linux-gnu
    pub fn host_triple(&self) -> &str {
        &self.rustc.host
    }

    pub fn target_triple(&self) -> &str {
        self.requested_target
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(self.host_triple())
    }

    pub fn json_messages(&self) -> bool {
        self.message_format == MessageFormat::Json
    }

    pub fn test(&self) -> bool {
        self.mode == CompileMode::Test || self.mode == CompileMode::Bench
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageFormat {
    Human,
    Json,
}

/// The general "mode" of what to do.
/// This is used for two purposes.  The commands themselves pass this in to
/// `compile_ws` to tell it the general execution strategy.  This influences
/// the default targets selected.  The other use is in the `Unit` struct
/// to indicate what is being done with a specific target.
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
pub enum CompileMode {
    /// A target being built for a test.
    Test,
    /// Building a target with `rustc` (lib or bin).
    Build,
    /// Building a target with `rustc` to emit `rmeta` metadata only. If
    /// `test` is true, then it is also compiled with `--test` to check it like
    /// a test.
    Check { test: bool },
    /// Used to indicate benchmarks should be built.  This is not used in
    /// `Target` because it is essentially the same as `Test` (indicating
    /// `--test` should be passed to rustc) and by using `Test` instead it
    /// allows some de-duping of Units to occur.
    Bench,
    /// A target that will be documented with `rustdoc`.
    /// If `deps` is true, then it will also document all dependencies.
    Doc { deps: bool },
    /// A target that will be tested with `rustdoc`.
    Doctest,
    /// A marker for Units that represent the execution of a `build.rs`
    /// script.
    RunCustomBuild,
}

impl CompileMode {
    /// Returns true if the unit is being checked.
    pub fn is_check(&self) -> bool {
        match *self {
            CompileMode::Check { .. } => true,
            _ => false,
        }
    }

    /// Returns true if this is a doc or doctest. Be careful using this.
    /// Although both run rustdoc, the dependencies for those two modes are
    /// very different.
    pub fn is_doc(&self) -> bool {
        match *self {
            CompileMode::Doc { .. } | CompileMode::Doctest => true,
            _ => false,
        }
    }

    /// Returns true if this is any type of test (test, benchmark, doctest, or
    /// check-test).
    pub fn is_any_test(&self) -> bool {
        match *self {
            CompileMode::Test
            | CompileMode::Bench
            | CompileMode::Check { test: true }
            | CompileMode::Doctest => true,
            _ => false,
        }
    }

    /// Returns true if this is the *execution* of a `build.rs` script.
    pub fn is_run_custom_build(&self) -> bool {
        *self == CompileMode::RunCustomBuild
    }

    /// List of all modes (currently used by `cargo clean -p` for computing
    /// all possible outputs).
    pub fn all_modes() -> &'static [CompileMode] {
        static ALL: [CompileMode; 9] = [
            CompileMode::Test,
            CompileMode::Build,
            CompileMode::Check { test: true },
            CompileMode::Check { test: false },
            CompileMode::Bench,
            CompileMode::Doc { deps: true },
            CompileMode::Doc { deps: false },
            CompileMode::Doctest,
            CompileMode::RunCustomBuild,
        ];
        &ALL
    }
}

/// Information required to build for a target
#[derive(Clone, Default)]
pub struct TargetConfig {
    /// The path of archiver (lib builder) for this target.
    pub ar: Option<PathBuf>,
    /// The path of the linker for this target.
    pub linker: Option<PathBuf>,
    /// Special build options for any necessary input files (filename -> options)
    pub overrides: HashMap<String, BuildOutput>,
}

impl TargetConfig {
    pub fn new(config: &Config, triple: &str) -> CargoResult<TargetConfig> {
        let key = format!("target.{}", triple);
        let mut ret = TargetConfig {
            ar: config.get_path(&format!("{}.ar", key))?.map(|v| v.val),
            linker: config.get_path(&format!("{}.linker", key))?.map(|v| v.val),
            overrides: HashMap::new(),
        };
        let table = match config.get_table(&key)? {
            Some(table) => table.val,
            None => return Ok(ret),
        };
        for (lib_name, value) in table {
            match lib_name.as_str() {
                "ar" | "linker" | "runner" | "rustflags" => continue,
                _ => {}
            }

            let mut output = BuildOutput {
                library_paths: Vec::new(),
                library_links: Vec::new(),
                cfgs: Vec::new(),
                env: Vec::new(),
                metadata: Vec::new(),
                rerun_if_changed: Vec::new(),
                rerun_if_env_changed: Vec::new(),
                warnings: Vec::new(),
            };
            // We require deterministic order of evaluation, so we must sort the pairs by key first.
            let mut pairs = Vec::new();
            for (k, value) in value.table(&lib_name)?.0 {
                pairs.push((k, value));
            }
            pairs.sort_by_key(|p| p.0);
            for (k, value) in pairs {
                let key = format!("{}.{}", key, k);
                match &k[..] {
                    "rustc-flags" => {
                        let (flags, definition) = value.string(k)?;
                        let whence = format!("in `{}` (in {})", key, definition.display());
                        let (paths, links) = BuildOutput::parse_rustc_flags(flags, &whence)?;
                        output.library_paths.extend(paths);
                        output.library_links.extend(links);
                    }
                    "rustc-link-lib" => {
                        let list = value.list(k)?;
                        output
                            .library_links
                            .extend(list.iter().map(|v| v.0.clone()));
                    }
                    "rustc-link-search" => {
                        let list = value.list(k)?;
                        output
                            .library_paths
                            .extend(list.iter().map(|v| PathBuf::from(&v.0)));
                    }
                    "rustc-cfg" => {
                        let list = value.list(k)?;
                        output.cfgs.extend(list.iter().map(|v| v.0.clone()));
                    }
                    "rustc-env" => for (name, val) in value.table(k)?.0 {
                        let val = val.string(name)?.0;
                        output.env.push((name.clone(), val.to_string()));
                    },
                    "warning" | "rerun-if-changed" | "rerun-if-env-changed" => {
                        bail!("`{}` is not supported in build script overrides", k);
                    }
                    _ => {
                        let val = value.string(k)?.0;
                        output.metadata.push((k.clone(), val.to_string()));
                    }
                }
            }
            ret.overrides.insert(lib_name, output);
        }

        Ok(ret)
    }
}
