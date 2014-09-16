#![warn(warnings)]
use std::collections::HashSet;
use std::io::File;

use serialize::{Encodable, Decodable};
use toml::{mod, Encoder};

use core::registry::PackageRegistry;
use core::{MultiShell, Source, Resolve, resolver, Package, SourceId};
use core::PackageId;
use sources::{PathSource};
use util::config::{Config};
use util::{CargoResult, human};
use util::toml as cargo_toml;

pub fn generate_lockfile(manifest_path: &Path,
                         shell: &mut MultiShell)
                         -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());
    let package = try!(source.get_root_package());
    let source_ids = package.get_source_ids();
    let mut config = try!(Config::new(shell, None, None));

    let resolve = {
        let mut registry = PackageRegistry::new(&mut config);
        try!(registry.add_sources(source_ids));
        try!(resolver::resolve(package.get_package_id(),
                               package.get_dependencies(),
                               &mut registry))
    };
    try!(write_resolve(&package, &resolve));
    Ok(())
}

pub fn update_lockfile(manifest_path: &Path,
                       shell: &mut MultiShell,
                       to_update: Option<String>) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());
    let package = try!(source.get_root_package());

    let lockfile = package.get_root().join("Cargo.lock");
    let source_id = package.get_package_id().get_source_id();
    let resolve = match try!(load_lockfile(&lockfile, source_id)) {
        Some(resolve) => resolve,
        None => return Err(human("A Cargo.lock must exist before it is updated"))
    };

    let mut config = try!(Config::new(shell, None, None));
    let mut registry = PackageRegistry::new(&mut config);

    let sources = match to_update {
        Some(name) => {
            let mut to_avoid = HashSet::new();
            for dep in resolve.iter().filter(|d| d.get_name() == name.as_slice()) {
                fill_with_deps(&resolve, dep, &mut to_avoid);
            }
            resolve.iter().filter(|pkgid| !to_avoid.contains(pkgid))
                   .map(|pkgid| pkgid.get_source_id().clone()).collect()
        }
        None => package.get_source_ids(),
    };
    try!(registry.add_sources(sources));

    let resolve = try!(resolver::resolve(package.get_package_id(),
                                         package.get_dependencies(),
                                         &mut registry));

    try!(write_resolve(&package, &resolve));
    return Ok(());

    fn fill_with_deps<'a>(resolve: &'a Resolve, dep: &'a PackageId,
                          set: &mut HashSet<&'a PackageId>) {
        if !set.insert(dep) { return }
        match resolve.deps(dep) {
            Some(mut deps) => {
                for dep in deps {
                    fill_with_deps(resolve, dep, set);
                }
            }
            None => {}
        }
    }
}

pub fn load_lockfile(path: &Path, sid: &SourceId) -> CargoResult<Option<Resolve>> {
    // If there is no lockfile, return none.
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(None)
    };

    let s = try!(f.read_to_string());

    let table = toml::Table(try!(cargo_toml::parse(s.as_slice(), path)));
    let mut d = toml::Decoder::new(table);
    let v: resolver::EncodableResolve = Decodable::decode(&mut d).unwrap();
    Ok(Some(try!(v.to_resolve(sid))))
}

pub fn write_resolve(pkg: &Package, resolve: &Resolve) -> CargoResult<()> {
    let loc = pkg.get_root().join("Cargo.lock");

    let mut e = Encoder::new();
    resolve.encode(&mut e).unwrap();

    let mut out = String::new();

    // Note that we do not use e.toml.to_string() as we want to control the
    // exact format the toml is in to ensure pretty diffs between updates to the
    // lockfile.
    let root = e.toml.find(&"root".to_string()).unwrap();

    out.push_str("[root]\n");
    emit_package(root.as_table().unwrap(), &mut out);

    let deps = e.toml.find(&"package".to_string()).unwrap().as_slice().unwrap();
    for dep in deps.iter() {
        let dep = dep.as_table().unwrap();

        out.push_str("[[package]]\n");
        emit_package(dep, &mut out);
    }

    try!(File::create(&loc).write_str(out.as_slice()));
    Ok(())
}

fn emit_package(dep: &toml::Table, out: &mut String) {
    out.push_str(format!("name = {}\n", lookup(dep, "name")).as_slice());
    out.push_str(format!("version = {}\n", lookup(dep, "version")).as_slice());

    dep.find(&"source".to_string()).map(|_| {
        out.push_str(format!("source = {}\n", lookup(dep, "source")).as_slice());
    });

    dep.find(&"dependencies".to_string()).map(|s| {
        let slice = s.as_slice().unwrap();

        if !slice.is_empty() {
            out.push_str("dependencies = [\n");

            for child in s.as_slice().unwrap().iter() {
                out.push_str(format!(" {},\n", child).as_slice());
            }

            out.push_str("]\n");
        }
        out.push_str("\n");
    });
}

fn lookup<'a>(table: &'a toml::Table, key: &str) -> &'a toml::Value {
    table.find(&key.to_string()).expect(format!("Didn't find {}", key).as_slice())
}
