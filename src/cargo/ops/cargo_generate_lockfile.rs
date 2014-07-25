use std::collections::TreeMap;
use std::io::fs::File;
use serialize::{Encodable, Decodable};
use toml;
use toml::{Encoder, Decoder};
use core::registry::PackageRegistry;
use core::{MultiShell, Source, Resolve, resolver};
use sources::{PathSource};
use util::config::{Config};
use util::{CargoResult};

pub fn generate_lockfile(manifest_path: &Path,
                         shell: &mut MultiShell,
                         update: bool)
                         -> CargoResult<()> {

    log!(4, "compile; manifest-path={}", manifest_path.display());

    let mut source = PathSource::for_path(&manifest_path.dir_path());
    try!(source.update());

    // TODO: Move this into PathSource
    let package = try!(source.get_root_package());
    debug!("loaded package; package={}", package);

    for key in package.get_manifest().get_unused_keys().iter() {
        try!(shell.warn(format!("unused manifest key: {}", key)));
    }

    let source_ids = package.get_source_ids();

    let resolve = {
        let mut config = try!(Config::new(shell, update, None, None));

        let mut registry =
            try!(PackageRegistry::new(source_ids, vec![], &mut config));

        try!(resolver::resolve(package.get_package_id(),
                               package.get_dependencies(),
                               &mut registry))
    };

    write_resolve(resolve);
    Ok(())
}

fn write_resolve(resolve: Resolve) {
    let mut e = Encoder::new();
    resolve.encode(&mut e).unwrap();

    let mut out = String::new();

    let root = e.toml.find(&"root".to_string()).unwrap();

    println!("root={}", root);

    out.push_str("[root]\n");
    emit_package(root.as_table().unwrap(), &mut out);

    let deps = e.toml.find(&"package".to_string()).unwrap().as_slice().unwrap();

    for dep in deps.iter() {
        let dep = dep.as_table().unwrap();

        out.push_str("[[package]]\n");
        emit_package(dep, &mut out);
    }

    let mut file = File::create(&Path::new("Cargo.lock"));
    write!(file, "{}", out);

    let mut d = Decoder::new(toml::Table(e.toml.clone()));
    let v: resolver::EncodableResolve = Decodable::decode(&mut d).unwrap();

    println!("{}", v);
}

fn emit_package(dep: &TreeMap<String, toml::Value>, out: &mut String) {
    out.push_str(format!("name = {}\n", lookup(dep, "name")).as_slice());
    out.push_str(format!("version = {}\n", lookup(dep, "version")).as_slice());

    dep.find(&"source".to_string()).map(|s| {
        out.push_str(format!("source = {}\n", lookup(dep, "source")).as_slice());
    });

    dep.find(&"dependencies".to_string()).map(|s| {
        let slice = s.as_slice().unwrap();

        if !slice.is_empty() {
            out.push_str("dependencies = [\n");

            for child in s.as_slice().unwrap().iter() {
                out.push_str(format!("  {},\n", child).as_slice());
            }

            out.push_str("]\n");
        }
        out.push_str("\n");
    });
}

fn lookup<'a>(table: &'a TreeMap<String, toml::Value>, key: &'static str) -> &'a toml::Value {
    table.find(&key.to_string()).expect(format!("Didn't find {}", key).as_slice())
}
