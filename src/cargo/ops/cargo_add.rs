use core::{SourceId, Workspace};

use ops::{self, cargo_install};
use sources::{SourceConfigMap};
use util::errors::{CargoResult};
use util::toml;

pub fn add(
    ws: &Workspace,
    krate: &str,
    source_id: &SourceId,
    vers: Option<&str>,
    opts: &ops::CompileOptions,
) -> CargoResult<()> {
    let cwd = ws.config().cwd();
    println!("cwd is {:?}", cwd);

    let map = SourceConfigMap::new(opts.config)?;

    let needs_update = true;

    let (pkg, _source) = cargo_install::select_pkg(
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
    )?;
    println!("pkg {:?}", pkg);
    let manifest_path = Some(toml::manifest::find(&Some(cwd.to_path_buf()))?);
    let mut manifest = toml::manifest::Manifest::open(&manifest_path)?;

    let dependency = toml::dependency::Dependency::new(&krate)
        .set_version(&format!("{}", pkg.version()));

    println!("dependency is {:?}", dependency);

    manifest.insert_into_table(&get_section(), &dependency)?;

    let mut file = toml::manifest::Manifest::find_file(&manifest_path)?;
    manifest.write_to_file(&mut file)?;
    Ok(())
}

fn get_section() -> Vec<String> {
    vec!["dependencies".to_owned()]
}
