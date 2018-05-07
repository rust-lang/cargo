use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use core::Workspace;
use ops;
use util::normalize_path;
use util::CargoResult;

/// Strongly typed options for the `cargo doc` command.
#[derive(Debug)]
pub struct DocOptions<'a> {
    /// Whether to attempt to open the browser after compiling the docs
    pub open_result: Option<String>,
    /// Options to pass through to the compiler
    pub compile_opts: ops::CompileOptions<'a>,
}

/// Main method for `cargo doc`.
pub fn doc(ws: &Workspace, options: &DocOptions) -> CargoResult<()> {
    let specs = options.compile_opts.spec.into_package_id_specs(ws)?;
    let resolve = ops::resolve_ws_precisely(
        ws,
        None,
        &options.compile_opts.features,
        options.compile_opts.all_features,
        options.compile_opts.no_default_features,
        &specs,
    )?;
    let (packages, resolve_with_overrides) = resolve;

    let pkgs = specs
        .iter()
        .map(|p| {
            let pkgid = p.query(resolve_with_overrides.iter())?;
            packages.get(pkgid)
        })
        .collect::<CargoResult<Vec<_>>>()?;

    let mut lib_names = HashMap::new();
    let mut bin_names = HashMap::new();
    for package in &pkgs {
        for target in package.targets().iter().filter(|t| t.documented()) {
            if target.is_lib() {
                if let Some(prev) = lib_names.insert(target.crate_name(), package) {
                    bail!(
                        "The library `{}` is specified by packages `{}` and \
                         `{}` but can only be documented once. Consider renaming \
                         or marking one of the targets as `doc = false`.",
                        target.crate_name(),
                        prev,
                        package
                    );
                }
            } else if let Some(prev) = bin_names.insert(target.crate_name(), package) {
                bail!(
                    "The binary `{}` is specified by packages `{}` and \
                     `{}` but can be documented only once. Consider renaming \
                     or marking one of the targets as `doc = false`.",
                    target.crate_name(),
                    prev,
                    package
                );
            }
        }
    }

    ops::compile(ws, &options.compile_opts)?;

    if let Some(ref module) = options.open_result {
        let name = if pkgs.len() > 1 {
            bail!(
                "Passing multiple packages and `open` is not supported.\n\
                 Please re-run this command with `-p <spec>` where `<spec>` \
                 is one of the following:\n  {}",
                pkgs.iter()
                    .map(|p| p.name().as_str())
                    .collect::<Vec<_>>()
                    .join("\n  ")
            );
        } else if pkgs.len() == 1 {
            pkgs[0].name().replace("-", "_")
        } else {
            match lib_names.keys().chain(bin_names.keys()).nth(0) {
                Some(s) => s.to_string(),
                None => return Ok(()),
            }
        };

        // Don't bother locking here as if this is getting deleted there's
        // nothing we can do about it and otherwise if it's getting overwritten
        // then that's also ok!
        let mut target_dir = ws.target_dir();
        if let Some(ref triple) = options.compile_opts.build_config.requested_target {
            target_dir.push(Path::new(triple).file_stem().unwrap());
        }

        target_dir = target_dir.join("doc").join(&name);
        let default_path = target_dir.join("index.html").into_path_unlocked();

        let path = if module.is_empty() {
            default_path
        } else {
            // A module path was provided so we try to convert it to a filesystem path
            let doc_root = target_dir.clone();
            let mut module_path = target_dir;
            let module_parts: Vec<&str> = module.split("::").collect();

            for part in module_parts.iter() {
                module_path = module_path.join(part);
            }

            let mut module_path = module_path.into_path_unlocked();

            // If the last segment of the module path ia a directory we use the index.html inside
            // otherwise we assume that it is a type and use the .t.html redirect page to avoid
            // trying all possible types
            module_path = if module_path.is_dir() {
                module_path.join("index.html")
            } else {
                let last_part = module_parts.last().unwrap();
                module_path.set_file_name(format!("{}.t.html", last_part));
                module_path
            };

            let mut shell = options.compile_opts.config.shell();
            if !module_path.exists() {
                shell.warn(format!(
                    "{} does not exist fallback to default path.",
                    module_path.display()
                ))?;

                default_path
            } else {
                // Resolve any possible path traversal operations to check if the generated path
                // is still within the doc directory
                module_path = normalize_path(&module_path);
                let doc_root = normalize_path(&doc_root.into_path_unlocked());

                if !module_path.starts_with(doc_root) {
                    shell.warn(format!(
                        "{} is outside of the doc directory fallback to default path.",
                        module_path.display()
                    ))?;

                    default_path
                } else {
                    module_path
                }
            }
        };

        if fs::metadata(&path).is_ok() {
            let mut shell = options.compile_opts.config.shell();
            shell.status("Opening", path.display())?;
            match open_docs(&path) {
                Ok(m) => shell.status("Launching", m)?,
                Err(e) => {
                    shell.warn("warning: could not determine a browser to open docs with, tried:")?;
                    for method in e {
                        shell.warn(format!("\t{}", method))?;
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_docs(path: &Path) -> Result<&'static str, Vec<&'static str>> {
    use std::env;
    let mut methods = Vec::new();
    // trying $BROWSER
    if let Ok(name) = env::var("BROWSER") {
        match Command::new(name).arg(path).status() {
            Ok(_) => return Ok("$BROWSER"),
            Err(_) => methods.push("$BROWSER"),
        }
    }

    for m in ["xdg-open", "gnome-open", "kde-open"].iter() {
        match Command::new(m).arg(path).status() {
            Ok(_) => return Ok(m),
            Err(_) => methods.push(m),
        }
    }

    Err(methods)
}

#[cfg(target_os = "windows")]
fn open_docs(path: &Path) -> Result<&'static str, Vec<&'static str>> {
    match Command::new("cmd").arg("/C").arg(path).status() {
        Ok(_) => Ok("cmd /C"),
        Err(_) => Err(vec!["cmd /C"]),
    }
}

#[cfg(target_os = "macos")]
fn open_docs(path: &Path) -> Result<&'static str, Vec<&'static str>> {
    match Command::new("open").arg(path).status() {
        Ok(_) => Ok("open"),
        Err(_) => Err(vec!["open"]),
    }
}
