use std::old_io::fs;
use std::env;

use ops::{self};
use util::{CargoResult, human, ChainError};
use core::source::Source;
use sources::PathSource;


pub fn install(manifest_path: &Path,
               name: Option<String>,
               prefix: Option<String>,
               options: &ops::CompileOptions) -> CargoResult<()> {
    let config = options.config;
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path(), config));
    try!(src.update());
    let root = try!(src.root_package());
    let env = options.env;
    let mut bins = root.manifest().targets().iter().filter(|a| {
        let matches_kind = a.is_bin();
        let matches_name = name.as_ref().map_or(true, |n| *n == a.name());
        matches_kind && matches_name && a.profile().env() == env &&
            !a.profile().is_custom_build()
    });
    let bin = try!(bins.next().chain_error(|| {
        match name.as_ref() {
            Some(name) => human(format!("no bin target named `{}` to install", name)),
            None => human("a bin target must be available for `cargo install`"),
        }
    }));
    let bin_name = format!("{}{}", bin.name(), env::consts::EXE_SUFFIX);
    match bins.next() {
        Some(..) => return Err(
            human("`cargo install` requires that a project only have one executable. \
                   Use the `--bin` option to specify which one to install")),
        None => {}
    }

    try!(ops::compile(manifest_path, options));
    let dst = manifest_path.dir_path().join("target");
    let dst = match options.target {
        Some(target) => dst.join(target),
        None => dst,
    };
    let exe = match bin.profile().dest() {
        Some(s) => dst.join(s).join(&bin_name),
        None => dst.join(&bin_name),
    };
    let exe = match exe.path_relative_from(config.cwd()) {
        Some(path) => path,
        None => exe,
    };

    let prefix = match (prefix, config.get_string("install.prefix")) {
        (Some(path), _) | (None, Ok(Some((path, _)))) => path,
        (None, _) => (try!(env::var("HOME")) + "/.local/bin/").to_string()
    };
    let install_dst = Path::new(prefix).join(bin_name);

    try!(fs::copy(&exe, &install_dst));

    try!(config.shell().status("Installed", format!("into {}", install_dst.display())));

    Ok(())
}
