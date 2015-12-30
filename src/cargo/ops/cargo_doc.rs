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

        let target_dir = options.compile_opts.config.target_dir(&package);
        let path = target_dir.join("doc").join(&name).join("index.html");
        if fs::metadata(&path).is_ok() {
            open_docs(&path);
        }
    }

    Ok(())
}

fn build_markdown_docs(manifest_path: &Path) -> CargoResult<()> {
    let docs_dir = manifest_path.parent().unwrap().join("doc");
    let target_dir = manifest_path.parent().unwrap().join("target/doc");

    try!(fs::create_dir_all(&target_dir));

    for entry in try!(fs::read_dir(docs_dir)) {
        let entry = try!(entry);
        let path = entry.path();

        let extension = match path.extension() {
            Some(e) => e,
            None => continue,
        };

        if "md" == extension {
            println!("let's do this: {:?}", path);
            let output_result = Command::new("rustdoc")
                .arg(&path)
                .arg("-otarget/doc")
                .output();
            let output = try!(output_result);

            if !output.status.success() {
                println!("failed");
            }
        }
    }

    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_docs(path: &Path) {
    // trying xdg-open
    match Command::new("xdg-open").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };

    // trying gnome-open
    match Command::new("gnome-open").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };

    // trying kde-open
    match Command::new("kde-open").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };
}

#[cfg(target_os = "windows")]
fn open_docs(path: &Path) {
    match Command::new("cmd").arg("/C").arg("start").arg("").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };
}

#[cfg(target_os = "macos")]
fn open_docs(path: &Path) {
    match Command::new("open").arg(path).status() {
        Ok(_) => return,
        Err(_) => ()
    };
}
