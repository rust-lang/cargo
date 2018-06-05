use core::{Package, SourceId}

use util::errors::{CargoResult};

fn select() -> CargoResult<Package> {
    let (pkg, source) = select_pkg(
            map.load(source_id)?,
            krate,
            vers,
            config,
            is_first_install,
            &mut |_| {
                bail!(
                    "must specify a crate to add from \
                     crates.io, or use --path or --git to \
                     specify alternate source"
                )
            },
        )?;

    pkg
}

pub fn select_pkg<'a, T>(
    mut source: T,
    name: Option<&str>,
    vers: Option<&str>,
    config: &Config,
    needs_update: bool,
    list_all: &mut FnMut(&mut T) -> CargoResult<Vec<Package>>,
) -> CargoResult<(Package, Box<Source + 'a>)>
where
    T: Source + 'a,
{
    
    }
}
