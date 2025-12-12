use crate::core::Workspace;
use crate::core::compiler::{Compilation, CompileKind};
use crate::core::shell::Verbosity;
use crate::ops;
use crate::util;
use crate::util::CargoResult;

use anyhow::{Error, bail};
use cargo_util::ProcessBuilder;

use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;

/// Format of rustdoc [`--output-format`][1].
///
/// [1]: https://doc.rust-lang.org/nightly/rustdoc/unstable-features.html#-w--output-format-output-format
#[derive(Debug, Default, Clone)]
pub enum OutputFormat {
    #[default]
    Html,
    Json,
}

impl OutputFormat {
    pub const POSSIBLE_VALUES: [&'static str; 2] = ["html", "json"];
}

impl FromStr for OutputFormat {
    // bail! error instead of string error like impl FromStr for Edition {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputFormat::Json),
            "html" => Ok(OutputFormat::Html),
            _ => bail!(
                "supported values for --output-format are `json` and `html`, \
						 but `{}` is unknown",
                s
            ),
        }
    }
}

/// Strongly typed options for the `cargo doc` command.
#[derive(Debug)]
pub struct DocOptions {
    /// Whether to attempt to open the browser after compiling the docs
    pub open_result: bool,
    /// Same as `rustdoc --output-format`
    pub output_format: OutputFormat,
    /// Options to pass through to the compiler
    pub compile_opts: ops::CompileOptions,
}

/// Main method for `cargo doc`.
pub fn doc(ws: &Workspace<'_>, options: &DocOptions) -> CargoResult<()> {
    let compilation = ops::compile(ws, &options.compile_opts)?;

    if ws.gctx().cli_unstable().rustdoc_mergeable_info {
        merge_cross_crate_info(ws, &compilation)?;
    }

    if options.open_result {
        let name = &compilation.root_crate_names.get(0).ok_or_else(|| {
            anyhow::anyhow!(
                "cannot open specified crate's documentation: no documentation generated"
            )
        })?;
        let kind = options.compile_opts.build_config.single_requested_kind()?;

        let path = path_by_output_format(&compilation, &kind, &name, &options.output_format);

        if path.exists() {
            util::open::open(&path, ws.gctx())?;
        }
    } else if ws.gctx().shell().verbosity() == Verbosity::Verbose {
        for name in &compilation.root_crate_names {
            for kind in &options.compile_opts.build_config.requested_kinds {
                let path =
                    path_by_output_format(&compilation, &kind, &name, &options.output_format);
                if path.exists() {
                    let mut shell = ws.gctx().shell();
                    let link = shell.err_file_hyperlink(&path);
                    shell.status("Generated", format!("{link}{}{link:#}", path.display()))?;
                }
            }
        }
    } else {
        let mut output = compilation.root_crate_names.iter().flat_map(|name| {
            options
                .compile_opts
                .build_config
                .requested_kinds
                .iter()
                .map(|kind| path_by_output_format(&compilation, kind, name, &options.output_format))
                .filter(|path| path.exists())
        });
        if let Some(first_path) = output.next() {
            let remaining = output.count();
            let remaining = match remaining {
                0 => "".to_owned(),
                1 => " and 1 other file".to_owned(),
                n => format!(" and {n} other files"),
            };

            let mut shell = ws.gctx().shell();
            let link = shell.err_file_hyperlink(&first_path);
            shell.status(
                "Generated",
                format!("{link}{}{link:#}{remaining}", first_path.display(),),
            )?;
        }
    }

    Ok(())
}

fn merge_cross_crate_info(ws: &Workspace<'_>, compilation: &Compilation<'_>) -> CargoResult<()> {
    let Some(fingerprints) = compilation.rustdoc_fingerprints.as_ref() else {
        return Ok(());
    };

    let now = std::time::Instant::now();
    for (kind, fingerprint) in fingerprints.iter() {
        let (target_name, build_dir, artifact_dir) = match kind {
            CompileKind::Host => ("host", ws.build_dir(), ws.target_dir()),
            CompileKind::Target(t) => {
                let name = t.short_name();
                let build_dir = ws.build_dir().join(name);
                let artifact_dir = ws.target_dir().join(name);
                (name, build_dir, artifact_dir)
            }
        };

        // rustdoc needs to read doc parts files from build dir
        build_dir.open_ro_shared_create(".cargo-lock", ws.gctx(), "build directory")?;
        // rustdoc will write to `<artifact-dir>/doc/`
        artifact_dir.open_rw_exclusive_create(".cargo-lock", ws.gctx(), "artifact directory")?;
        // We're leaking the layout implementation detail here.
        // This detail should be hidden when doc merge becomes a Unit of work inside the build.
        let rustdoc_artifact_dir = artifact_dir.join("doc");

        if !fingerprint.is_dirty() {
            ws.gctx().shell().verbose(|shell| {
                shell.status("Fresh", format_args!("doc-merge for {target_name}"))
            })?;
            continue;
        }

        fingerprint.persist(|doc_parts_dirs| {
            let mut cmd = ProcessBuilder::new(ws.gctx().rustdoc()?);
            if ws.gctx().extra_verbose() {
                cmd.display_env_vars();
            }
            cmd.retry_with_argfile(true);
            cmd.arg("-o")
                .arg(rustdoc_artifact_dir.as_path_unlocked())
                .arg("-Zunstable-options")
                .arg("--merge=finalize");
            for parts_dir in doc_parts_dirs {
                let mut include_arg = OsString::from("--include-parts-dir=");
                include_arg.push(parts_dir);
                cmd.arg(include_arg);
            }

            let num_crates = doc_parts_dirs.len();
            let plural = if num_crates == 1 { "" } else { "s" };

            ws.gctx().shell().status(
                "Merging",
                format_args!("{num_crates} doc{plural} for {target_name}"),
            )?;
            ws.gctx()
                .shell()
                .verbose(|shell| shell.status("Running", cmd.to_string()))?;
            cmd.exec()?;

            Ok(())
        })?;
    }

    let time_elapsed = util::elapsed(now.elapsed());
    ws.gctx().shell().status(
        "Finished",
        format_args!("documentation merge in {time_elapsed}"),
    )?;

    Ok(())
}

fn path_by_output_format(
    compilation: &Compilation<'_>,
    kind: &CompileKind,
    name: &str,
    output_format: &OutputFormat,
) -> PathBuf {
    if matches!(output_format, OutputFormat::Json) {
        compilation.root_output[kind]
            .with_file_name("doc")
            .join(format!("{}.json", name))
    } else {
        compilation.root_output[kind]
            .with_file_name("doc")
            .join(name)
            .join("index.html")
    }
}
