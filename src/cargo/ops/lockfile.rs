use std::old_io::File;

use rustc_serialize::{Encodable, Decodable};
use toml::{self, Encoder, Value};

use core::{Resolve, resolver, Package, SourceId};
use util::{CargoResult, ChainError, human};
use util::toml as cargo_toml;

pub fn load_pkg_lockfile(pkg: &Package) -> CargoResult<Option<Resolve>> {
    let lockfile = pkg.get_manifest_path().dir_path().join("Cargo.lock");
    let source_id = pkg.get_package_id().get_source_id();
    load_lockfile(&lockfile, source_id).chain_error(|| {
        human(format!("failed to parse lock file at: {}", lockfile.display()))
    })
}

pub fn load_lockfile(path: &Path, sid: &SourceId) -> CargoResult<Option<Resolve>> {
    // If there is no lockfile, return none.
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(None)
    };

    let s = try!(f.read_to_string());

    let table = toml::Value::Table(try!(cargo_toml::parse(s.as_slice(), path)));
    let mut d = toml::Decoder::new(table);
    let v: resolver::EncodableResolve = try!(Decodable::decode(&mut d));
    Ok(Some(try!(v.to_resolve(sid))))
}

pub fn write_pkg_lockfile(pkg: &Package, resolve: &Resolve) -> CargoResult<()> {
    let loc = pkg.get_root().join("Cargo.lock");
    write_lockfile(&loc, resolve)
}

pub fn write_lockfile(dst: &Path, resolve: &Resolve) -> CargoResult<()> {
    let mut e = Encoder::new();
    resolve.encode(&mut e).unwrap();

    let mut out = String::new();

    // Note that we do not use e.toml.to_string() as we want to control the
    // exact format the toml is in to ensure pretty diffs between updates to the
    // lockfile.
    let root = e.toml.get(&"root".to_string()).unwrap();

    out.push_str("[root]\n");
    emit_package(root.as_table().unwrap(), &mut out);

    let deps = e.toml.get(&"package".to_string()).unwrap().as_slice().unwrap();
    for dep in deps.iter() {
        let dep = dep.as_table().unwrap();

        out.push_str("[[package]]\n");
        emit_package(dep, &mut out);
    }

    match e.toml.get(&"metadata".to_string()) {
        Some(metadata) => {
            out.push_str("[metadata]\n");
            out.push_str(metadata.to_string().as_slice());
        }
        None => {}
    }

    try!(File::create(dst).write_str(out.as_slice()));
    Ok(())
}

fn emit_package(dep: &toml::Table, out: &mut String) {
    out.push_str(format!("name = {}\n", lookup(dep, "name")).as_slice());
    out.push_str(format!("version = {}\n", lookup(dep, "version")).as_slice());

    if dep.contains_key(&"source".to_string()) {
        out.push_str(format!("source = {}\n", lookup(dep, "source")).as_slice());
    }

    if let Some(ref s) = dep.get(&"dependencies".to_string()) {
        let slice = Value::as_slice(*s).unwrap();

        if !slice.is_empty() {
            out.push_str("dependencies = [\n");

            for child in slice.iter() {
                out.push_str(format!(" {},\n", child).as_slice());
            }

            out.push_str("]\n");
        }
        out.push_str("\n");
    }
}

fn lookup<'a>(table: &'a toml::Table, key: &str) -> &'a toml::Value {
    table.get(&key.to_string()).expect(format!("Didn't find {}", key).as_slice())
}
