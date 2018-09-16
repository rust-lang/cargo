use semver::{Version, VersionReq};

use core::{Dependency, Package, Source, SourceId, Workspace};

use ops;
use sources::{SourceConfigMap};
use util::{Config};
use util::errors::{CargoResult};

pub fn add(
    ws: &Workspace,
    krate: &str,
    source_id: &SourceId,
    vers: Option<&str>,
    opts: &ops::CompileOptions,
) -> CargoResult<()> {
    // println!("ws is {:?}", ws);
    println!("sourceid is {:?}", source_id);
    println!("vers are {:?}", vers);

    let map = SourceConfigMap::new(opts.config)?;

    let needs_update = true;

    let (pkg, source) = select_pkg(
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
    Ok(())
}

fn select_pkg<'a, T>(
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
    if needs_update {
        source.update()?;
    }

    match name {
        Some(name) => {
            let vers = match vers {
                Some(v) => {
                    // If the version begins with character <, >, =, ^, ~ parse it as a
                    // version range, otherwise parse it as a specific version
                    let first = v.chars()
                        .nth(0)
                        .ok_or_else(|| format_err!("no version provided for the `--vers` flag"))?;

                    match first {
                        '<' | '>' | '=' | '^' | '~' => match v.parse::<VersionReq>() {
                            Ok(v) => Some(v.to_string()),
                            Err(_) => bail!(
                                "the `--vers` provided, `{}`, is \
                                       not a valid semver version requirement\n\n
                                       Please have a look at \
                                       http://doc.crates.io/specifying-dependencies.html \
                                       for the correct format",
                                v
                            ),
                        },
                        _ => match v.parse::<Version>() {
                            Ok(v) => Some(format!("={}", v)),
                            Err(_) => {
                                let mut msg = format!(
                                    "\
                                     the `--vers` provided, `{}`, is \
                                     not a valid semver version\n\n\
                                     historically Cargo treated this \
                                     as a semver version requirement \
                                     accidentally\nand will continue \
                                     to do so, but this behavior \
                                     will be removed eventually",
                                    v
                                );

                                // If it is not a valid version but it is a valid version
                                // requirement, add a note to the warning
                                if v.parse::<VersionReq>().is_ok() {
                                    msg.push_str(&format!(
                                        "\nif you want to specify semver range, \
                                         add an explicit qualifier, like ^{}",
                                        v
                                    ));
                                }
                                config.shell().warn(&msg)?;
                                Some(v.to_string())
                            }
                        },
                    }
                }
                None => None,
            };
            let vers = vers.as_ref().map(|s| &**s);
            let vers_spec = if vers.is_none() && source.source_id().is_registry() {
                // Avoid pre-release versions from crate.io
                // unless explicitly asked for
                Some("*")
            } else {
                vers
            };
            let dep = Dependency::parse_no_deprecated(
                name,
                vers_spec,
                source.source_id(),
            )?;
            let deps = source.query_vec(&dep)?;
            match deps.iter().map(|p| p.package_id()).max() {
                Some(pkgid) => {
                    let pkg = source.download(pkgid)?;
                    Ok((pkg, Box::new(source)))
                }
                None => {
                    let vers_info = vers.map(|v| format!(" with version `{}`", v))
                        .unwrap_or_default();
                    Err(format_err!(
                        "could not find `{}` in {}{}",
                        name,
                        source.source_id(),
                        vers_info
                    ))
                }
            }
        }
        None => {
            let candidates = list_all(&mut source)?;
            let binaries = candidates
                .iter()
                .filter(|cand| cand.targets().iter().filter(|t| t.is_bin()).count() > 0);
            let examples = candidates
                .iter()
                .filter(|cand| cand.targets().iter().filter(|t| t.is_example()).count() > 0);
            let pkg = match one(binaries, |v| multi_err("binaries", v))? {
                Some(p) => p,
                None => match one(examples, |v| multi_err("examples", v))? {
                    Some(p) => p,
                    None => bail!(
                        "no packages found with binaries or \
                         examples"
                    ),
                },
            };
            return Ok((pkg.clone(), Box::new(source)));

            fn multi_err(kind: &str, mut pkgs: Vec<&Package>) -> String {
                pkgs.sort_unstable_by_key(|a| a.name());
                format!(
                    "multiple packages with {} found: {}",
                    kind,
                    pkgs.iter()
                        .map(|p| p.name().as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
    }
}

fn one<I, F>(mut i: I, f: F) -> CargoResult<Option<I::Item>>
where
    I: Iterator,
    F: FnOnce(Vec<I::Item>) -> String,
{
    match (i.next(), i.next()) {
        (Some(i1), Some(i2)) => {
            let mut v = vec![i1, i2];
            v.extend(i);
            Err(format_err!("{}", f(v)))
        }
        (Some(i), None) => Ok(Some(i)),
        (None, _) => Ok(None),
    }
}
