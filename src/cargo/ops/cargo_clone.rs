use std::path::{Path, PathBuf};
use std::fs;
use std::env;

use util::Config;
use core::{Source, SourceId};
use sources::{GitSource, PathSource, SourceConfigMap};
use util::errors::{CargoResult, CargoResultExt};
use ops::select_pkg;


pub struct CloneOptions<'a> {
    pub config: &'a Config,
    /// Crate's name
    pub name: Option<&'a str>,
    /// Source ID to clone from
    pub source_id: SourceId,
    /// Path to clone to
    pub prefix: Option<&'a str>,
    /// Overwrite an existing directory with crate's content
    pub force: bool,
    /// Crate's version
    pub version: Option<&'a str>,
}

pub fn clone(opts: CloneOptions) -> CargoResult<()> {
    let CloneOptions {
        config,
        name,
        source_id,
        prefix,
        force,
        version,
    } = opts;

    let (pkg, _) = if source_id.is_path() {
        let path = source_id
            .url()
            .to_file_path()
            .map_err(|()| format_err!("path sources must have a valid path"))?;
        let mut src = PathSource::new(&path, &source_id, config);
        src.update().chain_err(|| {
            format_err!(
                "`{}` is not a crate root; specify a crate to \
                 install from crates.io, or use --path or --git to \
                 specify an alternate source",
                path.display()
            )
        })?;

        select_pkg(src, name, version, config, true, &mut |path| path.read_packages())?
    } else if source_id.is_git() {
        select_pkg(GitSource::new(&source_id, config)?,
                   name, version, config, true, &mut |git| git.read_packages())?
    } else {
        let map = SourceConfigMap::new(config)?;
        select_pkg(map.load(&source_id)?,
                   name, version, config, true,
                   &mut |_| {
                       bail!("must specify a crate to clone from \
                              crates.io, or use --path or --git to \
                              specify alternate source")
                   }
        )?
    };

    config.shell().status("Cloning", &pkg)?;

    // If prefix was not supplied, clone into current dir
    let mut dest_path = match prefix {
        Some(prefix) => PathBuf::from(prefix),
        None => env::current_dir()?,
    };

    dest_path.push(pkg.name().as_str());

    if dest_path.exists() {
        if force {
            config.shell().status("Replacing", dest_path.display())?;
            fs::remove_dir_all(&dest_path)?;
        } else {
            bail!(format!("Directory `{}` already exists. Add --force to overwrite",
                          dest_path.display()));
        }
    }

    clone_directory(&pkg.root(), &dest_path)
}

fn clone_directory(from: &Path, to: &Path) -> CargoResult<()> {
    fs::create_dir_all(&to)?;

    let entries = from.read_dir()
        .chain_err(|| format!("failed to read directory `{}`", from.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let mut to = to.to_owned();
        to.push(path.strip_prefix(from)?);

        if path.is_file() && &entry.file_name() != ".cargo-ok" {
            // .cargo-ok is not wanted in this context
            fs::copy(&path, &to)?;
        } else if path.is_dir() {
            fs::create_dir(&to)?;
            clone_directory(&path, &to)?
        }
    }

    Ok(())
}