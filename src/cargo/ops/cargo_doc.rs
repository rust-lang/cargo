use crate::core::compiler::{Compilation, CompileKind};
use crate::core::{Shell, Workspace};
use crate::ops;
use crate::util::config::{Config, PathAndArgs};
use crate::util::CargoResult;
use anyhow::{bail, Error};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
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

    if options.open_result {
        let name = &compilation
            .root_crate_names
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("no crates with documentation"))?;
        let kind = options.compile_opts.build_config.single_requested_kind()?;

        let path = path_by_output_format(&compilation, &kind, &name, &options.output_format);

        if path.exists() {
            let config_browser = {
                let cfg: Option<PathAndArgs> = ws.config().get("doc.browser")?;
                cfg.map(|path_args| (path_args.path.resolve_program(ws.config()), path_args.args))
            };
            let mut shell = ws.config().shell();
            let link = shell.err_file_hyperlink(&path);
            shell.status(
                "Opening",
                format!("{}{}{}", link.open(), path.display(), link.close()),
            )?;
            open_docs(&path, &mut shell, config_browser, ws.config())?;
        }
    } else {
        for name in &compilation.root_crate_names {
            for kind in &options.compile_opts.build_config.requested_kinds {
                let path =
                    path_by_output_format(&compilation, &kind, &name, &options.output_format);
                if path.exists() {
                    let mut shell = ws.config().shell();
                    let link = shell.err_file_hyperlink(&path);
                    shell.status(
                        "Generated",
                        format!("{}{}{}", link.open(), path.display(), link.close()),
                    )?;
                }
            }
        }
    }

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

fn open_docs(
    path: &Path,
    shell: &mut Shell,
    config_browser: Option<(PathBuf, Vec<String>)>,
    config: &Config,
) -> CargoResult<()> {
    let browser =
        config_browser.or_else(|| Some((PathBuf::from(config.get_env_os("BROWSER")?), Vec::new())));

    match browser {
        Some((browser, initial_args)) => {
            if let Err(e) = Command::new(&browser).args(initial_args).arg(path).status() {
                shell.warn(format!(
                    "Couldn't open docs with {}: {}",
                    browser.to_string_lossy(),
                    e
                ))?;
            }
        }
        None => {
            if let Err(e) = opener::open(&path) {
                let e = e.into();
                crate::display_warning_with_error("couldn't open docs", &e, shell);
            }
        }
    };

    Ok(())
}
