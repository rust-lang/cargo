use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use core::{Package, PackageIdSpec};
use ops;
use util::CargoResult;

pub struct DocOptions<'a> {
    pub open_result: bool,
    pub compile_opts: ops::CompileOptions<'a>,
}

pub fn doc(manifest_path: &Path,
           options: &DocOptions) -> CargoResult<()> {
    let package = try!(Package::for_path(manifest_path, options.compile_opts.config));

    let mut lib_names = HashSet::new();
    let mut bin_names = HashSet::new();
    if options.compile_opts.spec.is_empty() {
        for target in package.targets().iter().filter(|t| t.documented()) {
            if target.is_lib() {
                assert!(lib_names.insert(target.crate_name()));
            } else {
                assert!(bin_names.insert(target.crate_name()));
            }
        }
        for bin in bin_names.iter() {
            if lib_names.contains(bin) {
                bail!("cannot document a package where a library and a binary \
                       have the same name. Consider renaming one or marking \
                       the target as `doc = false`")
            }
        }
    }

    try!(ops::compile(manifest_path, &options.compile_opts));
    try!(build_markdown_docs(manifest_path));

    if options.open_result {
        let name = if options.compile_opts.spec.len() > 1 {
            bail!("Passing multiple packages and `open` is not supported")
        } else if options.compile_opts.spec.len() == 1 {
            try!(PackageIdSpec::parse(&options.compile_opts.spec[0]))
                                             .name().replace("-", "_")
        } else {
            match lib_names.iter().chain(bin_names.iter()).nth(0) {
                Some(s) => s.to_string(),
                None => return Ok(())
            }
        };

        // Don't bother locking here as if this is getting deleted there's
        // nothing we can do about it and otherwise if it's getting overwritten
        // then that's also ok!
        let target_dir = options.compile_opts.config.target_dir(&package);
        let path = target_dir.join("doc").join(&name).join("index.html");
        let path = path.into_path_unlocked();
        if fs::metadata(&path).is_ok() {
            let mut shell = options.compile_opts.config.shell();
            match open_docs(&path) {
                Ok(m) => try!(shell.status("Launching", m)),
                Err(e) => {
                    try!(shell.warn(
                            "warning: could not determine a browser to open docs with, tried:"));
                    for method in e {
                        try!(shell.warn(format!("\t{}", method)));
                    }
                }
            }
        }
    }

    Ok(())
}

fn walk_through_dirs(read_dir: &Path, output_path: &Path) -> CargoResult<()> {
    let mut dir_list = vec!();

    for entry in try!(fs::read_dir(read_dir)) {
        let entry = try!(entry);
        let path = entry.path();

        if path.is_dir() {
            dir_list.push(path);
        } else {
            let extension = match path.extension() {
                Some(e) => e,
                None => continue,
            };

            if "md" == extension {
                if !output_path.exists() {
                    try!(fs::create_dir_all(&output_path));
                }
                let output_result = Command::new("rustdoc")
                    .arg(&path)
                    .arg(&format!("-o{}", output_path.to_str().unwrap_or("target/doc")))
                    .output();
                let output = try!(output_result);

                if !output.status.success() {
                    println!("failed");
                }
            }
        }
    }
    if dir_list.len() < 1 {
        Ok(())
    } else {
        // this code can be multithreaded if needed
        for dir in dir_list.iter().skip(1) {
            if let Some(rel) = dir.file_name() {
                let output_path = output_path.join(rel);
                try!(walk_through_dirs(dir, output_path.as_path()));
            }
        }
        for dir in dir_list[0..1].iter() {
            if let Some(rel) = dir.file_name() {
                let output_path = output_path.join(rel);
                return walk_through_dirs(&dir_list[0], output_path.as_path());
            }
        }
        Ok(())
    }
}

fn build_markdown_docs(manifest_path: &Path) -> CargoResult<()> {
    let docs_dir = if let Some(dir) = manifest_path.parent() {
        dir.join("doc")
    } else {
        return Ok(());
    };

    let target_dir = if let Some(dir) = manifest_path.parent() {
        dir.join("target/doc")
    } else {
        return Ok(());
    };

    try!(fs::create_dir_all(&target_dir));

    walk_through_dirs(&docs_dir, Path::new("target/doc"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_docs(path: &Path) -> Result<&'static str, Vec<&'static str>> {
    use std::env;
    let mut methods = Vec::new();
    // trying $BROWSER
    match env::var("BROWSER"){
        Ok(name) => match Command::new(name).arg(path).status() {
            Ok(_) => return Ok("$BROWSER"),
            Err(_) => methods.push("$BROWSER")
        },
        Err(_) => () // Do nothing here if $BROWSER is not found
    }

    for m in ["xdg-open", "gnome-open", "kde-open"].iter() {
        match Command::new(m).arg(path).status() {
            Ok(_) => return Ok(m),
            Err(_) => methods.push(m)
        }
    }

    Err(methods)
}

#[cfg(target_os = "windows")]
fn open_docs(path: &Path) -> Result<&'static str, Vec<&'static str>> {
    match Command::new("cmd").arg("/C").arg("start").arg("").arg(path).status() {
        Ok(_) => return Ok("cmd /C start"),
        Err(_) => return Err(vec!["cmd /C start"])
    };
}

#[cfg(target_os = "macos")]
fn open_docs(path: &Path) -> Result<&'static str, Vec<&'static str>> {
    match Command::new("open").arg(path).status() {
        Ok(_) => return Ok("open"),
        Err(_) => return Err(vec!["open"])
    };
}
