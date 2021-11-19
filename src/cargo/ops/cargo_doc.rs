use crate::core::{Shell, Workspace};
use crate::ops;
use crate::util::config::PathAndArgs;
use crate::util::{CargoResult, Filesystem};
use cargo_util::paths;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Strongly typed options for the `cargo doc` command.
#[derive(Debug)]
pub struct DocOptions {
    /// Whether to attempt to open the browser after compiling the docs
    pub open_result: bool,
    /// Directory to copy the generated documentation to
    pub publish_dir: Option<Filesystem>,
    /// Options to pass through to the compiler
    pub compile_opts: ops::CompileOptions,
}

/// Main method for `cargo doc`.
pub fn doc(ws: &Workspace<'_>, options: &DocOptions) -> CargoResult<()> {
    let compilation = ops::compile(ws, &options.compile_opts)?;
    let kind = options.compile_opts.build_config.single_requested_kind()?;

    if let Some(publish_dir) = &options.publish_dir {
        let mut shell = ws.config().shell();
        shell.status("Publishing", publish_dir.display())?;

        let publish_dir = publish_dir.as_path_unlocked();
        paths::create_dir_all(publish_dir)?;

        let doc_path = compilation.root_output[&kind].with_file_name("doc");
        let doc_entries = walkdir::WalkDir::new(&doc_path)
            .into_iter()
            .filter_map(|e| e.ok());

        for entry in doc_entries {
            let from = entry.path();
            let to = publish_dir.join(from.strip_prefix(&doc_path)?);

            if entry.file_type().is_dir() {
                paths::create_dir_all(to)?;
            } else {
                paths::copy(from, to)?;
            }
        }
    }

    if options.open_result {
        let name = &compilation.root_crate_names[0];
        let path = compilation.root_output[&kind]
            .with_file_name("doc")
            .join(&name)
            .join("index.html");
        if path.exists() {
            let config_browser = {
                let cfg: Option<PathAndArgs> = ws.config().get("doc.browser")?;
                cfg.map(|path_args| (path_args.path.resolve_program(ws.config()), path_args.args))
            };

            let mut shell = ws.config().shell();
            shell.status("Opening", path.display())?;
            open_docs(&path, &mut shell, config_browser)?;
        }
    }

    Ok(())
}

fn open_docs(
    path: &Path,
    shell: &mut Shell,
    config_browser: Option<(PathBuf, Vec<String>)>,
) -> CargoResult<()> {
    let browser =
        config_browser.or_else(|| Some((PathBuf::from(std::env::var_os("BROWSER")?), Vec::new())));

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
