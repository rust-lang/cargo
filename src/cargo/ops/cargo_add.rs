use core::{SourceId, Workspace};

use ops::{self, cargo_install};
use sources::{GitSource, SourceConfigMap};
use util::errors::{CargoResult};
use util::toml;

pub fn add(
    ws: &Workspace,
    krates: Vec<&str>,
    source_id: &SourceId,
    vers: Option<&str>,
    opts: &ops::CompileOptions,
) -> CargoResult<()> {
    let cwd = ws.config().cwd();
    println!("cwd is {:?}", cwd);

    let map = SourceConfigMap::new(opts.config)?;

    let manifest_path = Some(toml::manifest::find(&Some(cwd.to_path_buf()))?);
    let mut manifest = toml::manifest::Manifest::open(&manifest_path)?;

    let mut needs_update = true;
    for krate in krates {
        add_one(
            &map,
            krate,
            source_id,
            vers,
            opts,
            needs_update,
            &mut manifest,
        )?;
        needs_update = false;
    }

    let mut file = toml::manifest::Manifest::find_file(&manifest_path)?;
    manifest.write_to_file(&mut file)?;
    
    Ok(())
}

fn add_one(
    map: &SourceConfigMap,
    krate: &str,
    source_id: &SourceId,
    vers: Option<&str>,
    opts: &ops::CompileOptions,
    needs_update: bool,
    manifest: &mut toml::manifest::Manifest,
    ) -> CargoResult<()> {
    let (pkg, _source) = if source_id.is_git() {
        cargo_install::select_pkg(
            GitSource::new(source_id, opts.config)?,
            Some(krate),
            vers,
            opts.config,
            needs_update,
            &mut |_| {
                bail!(
                    "must specify a crate to install from \
                     crates.io, or use --path or --git to \
                     specify alternate source"
                )
            },
        )?
    } else {
        cargo_install::select_pkg(
            map.load(source_id)?,
            Some(krate),
            vers,
            opts.config,
            needs_update,
            &mut |_| {
                bail!(
                    "must specify a crate to install from \
                     crates.io, or use --path or --git to \
                     specify alternate source"
                )
            },
        )?
    };
    println!("pkg {:?}", pkg);
    let package_id = pkg.package_id();
    println!("package_id is {:?}", package_id);

    let mut dependency = toml::dependency::Dependency::new(&krate)
        .set_version(&format!("{}", package_id.version()));

    if source_id.is_git() {
        dependency = dependency.set_path(&format!("{}", package_id.source_id()));
    }

    println!("is git {}", source_id.is_git());
    println!("dependency is {:?}", dependency);

    manifest.insert_into_table(&get_section(), &dependency)?;

    Ok(())
}

fn get_section() -> Vec<String> {
    vec!["dependencies".to_owned()]
}
