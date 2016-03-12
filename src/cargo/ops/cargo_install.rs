use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use toml;

use core::{SourceId, Source, Package, Registry, Dependency, PackageIdSpec};
use core::PackageId;
use ops::{self, CompileFilter};
use sources::{GitSource, PathSource, RegistrySource};
use util::{CargoResult, ChainError, Config, human, internal};
use util::{Filesystem, FileLock};

#[derive(RustcDecodable, RustcEncodable)]
enum CrateListing {
    V1(CrateListingV1),
    Empty,
}

#[derive(RustcDecodable, RustcEncodable)]
struct CrateListingV1 {
    v1: BTreeMap<PackageId, BTreeSet<String>>,
}

struct Transaction {
    bins: Vec<PathBuf>,
}

impl Drop for Transaction {
    fn drop(&mut self) {
        for bin in self.bins.iter() {
            let _ = fs::remove_file(bin);
        }
    }
}

pub fn install(root: Option<&str>,
               krate: Option<&str>,
               source_id: &SourceId,
               vers: Option<&str>,
               opts: &ops::CompileOptions) -> CargoResult<()> {
    let config = opts.config;
    let root = try!(resolve_root(root, config));
    let (pkg, source) = if source_id.is_git() {
        try!(select_pkg(GitSource::new(source_id, config), source_id,
                        krate, vers, &mut |git| git.read_packages()))
    } else if source_id.is_path() {
        let path = source_id.url().to_file_path().ok()
                            .expect("path sources must have a valid path");
        let mut src = PathSource::new(&path, source_id, config);
        try!(src.update().chain_error(|| {
            human(format!("`{}` is not a crate root; specify a crate to \
                           install from crates.io, or use --path or --git to \
                           specify an alternate source", path.display()))
        }));
        try!(select_pkg(PathSource::new(&path, source_id, config),
                        source_id, krate, vers,
                        &mut |path| path.read_packages()))
    } else {
        try!(select_pkg(RegistrySource::new(source_id, config),
                        source_id, krate, vers,
                        &mut |_| Err(human("must specify a crate to install from \
                                            crates.io, or use --path or --git to \
                                            specify alternate source"))))
    };

    // Preflight checks to check up front whether we'll overwrite something.
    // We have to check this again afterwards, but may as well avoid building
    // anything if we're gonna throw it away anyway.
    {
        let metadata = try!(metadata(config, &root));
        let list = try!(read_crate_list(metadata.file()));
        let dst = metadata.parent().join("bin");
        try!(check_overwrites(&dst, &pkg, &opts.filter, &list));
    }

    let target_dir = if source_id.is_path() {
        config.target_dir(&pkg)
    } else {
        config.cwd().join("target-install")
    };
    config.set_target_dir(&target_dir);
    let compile = try!(ops::compile_pkg(&pkg, Some(source), opts).chain_error(|| {
        human(format!("failed to compile `{}`, intermediate artifacts can be \
                       found at `{}`", pkg, target_dir.display()))
    }));

    let metadata = try!(metadata(config, &root));
    let mut list = try!(read_crate_list(metadata.file()));
    let dst = metadata.parent().join("bin");
    try!(check_overwrites(&dst, &pkg, &opts.filter, &list));

    let mut t = Transaction { bins: Vec::new() };
    try!(fs::create_dir_all(&dst));
    for bin in compile.binaries.iter() {
        let dst = dst.join(bin.file_name().unwrap());
        try!(config.shell().status("Installing", dst.display()));
        try!(fs::copy(&bin, &dst).chain_error(|| {
            human(format!("failed to copy `{}` to `{}`", bin.display(),
                          dst.display()))
        }));
        t.bins.push(dst);
    }

    if !source_id.is_path() {
        try!(fs::remove_dir_all(&target_dir));
    }

    list.v1.entry(pkg.package_id().clone()).or_insert_with(|| {
        BTreeSet::new()
    }).extend(t.bins.iter().map(|t| {
        t.file_name().unwrap().to_string_lossy().into_owned()
    }));
    try!(write_crate_list(metadata.file(), list));

    t.bins.truncate(0);

    // Print a warning that if this directory isn't in PATH that they won't be
    // able to run these commands.
    let path = env::var_os("PATH").unwrap_or(OsString::new());
    for path in env::split_paths(&path) {
        if path == dst {
            return Ok(())
        }
    }

    try!(config.shell().warn(&format!("be sure to add `{}` to your PATH to be \
                                       able to run the installed binaries",
                                      dst.display())));
    Ok(())
}

fn select_pkg<'a, T>(mut source: T,
                     source_id: &SourceId,
                     name: Option<&str>,
                     vers: Option<&str>,
                     list_all: &mut FnMut(&mut T) -> CargoResult<Vec<Package>>)
                     -> CargoResult<(Package, Box<Source + 'a>)>
    where T: Source + 'a
{
    try!(source.update());
    match name {
        Some(name) => {
            let dep = try!(Dependency::parse(name, vers, source_id));
            let deps = try!(source.query(&dep));
            match deps.iter().map(|p| p.package_id()).max() {
                Some(pkgid) => {
                    let pkg = try!(source.download(pkgid));
                    Ok((pkg, Box::new(source)))
                }
                None => {
                    let vers_info = vers.map(|v| format!(" with version `{}`", v))
                                        .unwrap_or(String::new());
                    Err(human(format!("could not find `{}` in `{}`{}", name,
                                      source_id, vers_info)))
                }
            }
        }
        None => {
            let candidates = try!(list_all(&mut source));
            let binaries = candidates.iter().filter(|cand| {
                cand.targets().iter().filter(|t| t.is_bin()).count() > 0
            });
            let examples = candidates.iter().filter(|cand| {
                cand.targets().iter().filter(|t| t.is_example()).count() > 0
            });
            let pkg = match try!(one(binaries, |v| multi_err("binaries", v))) {
                Some(p) => p,
                None => {
                    match try!(one(examples, |v| multi_err("examples", v))) {
                        Some(p) => p,
                        None => bail!("no packages found with binaries or \
                                       examples"),
                    }
                }
            };
            return Ok((pkg.clone(), Box::new(source)));

            fn multi_err(kind: &str, mut pkgs: Vec<&Package>) -> String {
                pkgs.sort_by(|a, b| a.name().cmp(b.name()));
                format!("multiple packages with {} found: {}", kind,
                        pkgs.iter().map(|p| p.name()).collect::<Vec<_>>()
                            .join(", "))
            }
        }
    }
}

fn one<I, F>(mut i: I, f: F) -> CargoResult<Option<I::Item>>
    where I: Iterator,
          F: FnOnce(Vec<I::Item>) -> String
{
    match (i.next(), i.next()) {
        (Some(i1), Some(i2)) => {
            let mut v = vec![i1, i2];
            v.extend(i);
            Err(human(f(v)))
        }
        (Some(i), None) => Ok(Some(i)),
        (None, _) => Ok(None)
    }
}

fn check_overwrites(dst: &Path,
                    pkg: &Package,
                    filter: &ops::CompileFilter,
                    prev: &CrateListingV1) -> CargoResult<()> {
    let check = |name| {
        let name = format!("{}{}", name, env::consts::EXE_SUFFIX);
        if fs::metadata(dst.join(&name)).is_err() {
            return Ok(())
        }
        let mut msg = format!("binary `{}` already exists in destination", name);
        if let Some((p, _)) = prev.v1.iter().find(|&(_, v)| v.contains(&name)) {
            msg.push_str(&format!(" as part of `{}`", p));
        }
        Err(human(msg))
    };
    match *filter {
        CompileFilter::Everything => {
            // If explicit --bin or --example flags were passed then those'll
            // get checked during cargo_compile, we only care about the "build
            // everything" case here
            if pkg.targets().iter().filter(|t| t.is_bin()).next().is_none() {
                bail!("specified package has no binaries")
            }

            for target in pkg.targets().iter().filter(|t| t.is_bin()) {
                try!(check(target.name()));
            }
        }
        CompileFilter::Only { bins, examples, .. } => {
            for bin in bins.iter().chain(examples) {
                try!(check(bin));
            }
        }
    }
    Ok(())
}

fn read_crate_list(mut file: &File) -> CargoResult<CrateListingV1> {
    (|| -> CargoResult<_> {
        let mut contents = String::new();
        try!(file.read_to_string(&mut contents));
        let listing = try!(toml::decode_str(&contents).chain_error(|| {
            internal("invalid TOML found for metadata")
        }));
        match listing {
            CrateListing::V1(v1) => Ok(v1),
            CrateListing::Empty => {
                Ok(CrateListingV1 { v1: BTreeMap::new() })
            }
        }
    }).chain_error(|| {
        human("failed to parse crate metadata")
    })
}

fn write_crate_list(mut file: &File, listing: CrateListingV1) -> CargoResult<()> {
    (|| -> CargoResult<_> {
        try!(file.seek(SeekFrom::Start(0)));
        try!(file.set_len(0));
        let data = toml::encode_str::<CrateListing>(&CrateListing::V1(listing));
        try!(file.write_all(data.as_bytes()));
        Ok(())
    }).chain_error(|| {
        human("failed to write crate metadata")
    })
}

pub fn install_list(dst: Option<&str>, config: &Config) -> CargoResult<()> {
    let dst = try!(resolve_root(dst, config));
    let dst = try!(metadata(config, &dst));
    let list = try!(read_crate_list(dst.file()));
    let mut shell = config.shell();
    let out = shell.out();
    for (k, v) in list.v1.iter() {
        try!(writeln!(out, "{}:", k));
        for bin in v {
            try!(writeln!(out, "    {}", bin));
        }
    }
    Ok(())
}

pub fn uninstall(root: Option<&str>,
                 spec: &str,
                 bins: &[String],
                 config: &Config) -> CargoResult<()> {
    let root = try!(resolve_root(root, config));
    let crate_metadata = try!(metadata(config, &root));
    let mut metadata = try!(read_crate_list(crate_metadata.file()));
    let mut to_remove = Vec::new();
    {
        let result = try!(PackageIdSpec::query_str(spec, metadata.v1.keys()))
                                        .clone();
        let mut installed = match metadata.v1.entry(result.clone()) {
            Entry::Occupied(e) => e,
            Entry::Vacant(..) => panic!("entry not found: {}", result),
        };
        let dst = crate_metadata.parent().join("bin");
        for bin in installed.get() {
            let bin = dst.join(bin);
            if fs::metadata(&bin).is_err() {
                bail!("corrupt metadata, `{}` does not exist when it should",
                      bin.display())
            }
        }

        let bins = bins.iter().map(|s| {
            if s.ends_with(env::consts::EXE_SUFFIX) {
                s.to_string()
            } else {
                format!("{}{}", s, env::consts::EXE_SUFFIX)
            }
        }).collect::<Vec<_>>();

        for bin in bins.iter() {
            if !installed.get().contains(bin) {
                bail!("binary `{}` not installed as part of `{}`", bin, result)
            }
        }

        if bins.is_empty() {
            to_remove.extend(installed.get().iter().map(|b| dst.join(b)));
            installed.get_mut().clear();
        } else {
            for bin in bins.iter() {
                to_remove.push(dst.join(bin));
                installed.get_mut().remove(bin);
            }
        }
        if installed.get().is_empty() {
            installed.remove();
        }
    }
    try!(write_crate_list(crate_metadata.file(), metadata));
    for bin in to_remove {
        try!(config.shell().status("Removing", bin.display()));
        try!(fs::remove_file(bin));
    }

    Ok(())
}

fn metadata(config: &Config, root: &Filesystem) -> CargoResult<FileLock> {
    root.open_rw(Path::new(".crates.toml"), config, "crate metadata")
}

fn resolve_root(flag: Option<&str>,
                config: &Config) -> CargoResult<Filesystem> {
    let config_root = try!(config.get_path("install.root"));
    Ok(flag.map(PathBuf::from).or_else(|| {
        env::var_os("CARGO_INSTALL_ROOT").map(PathBuf::from)
    }).or_else(move || {
        config_root.map(|v| v.val)
    }).map(Filesystem::new).unwrap_or_else(|| {
        config.home().clone()
    }))
}
