use std::io::prelude::*;

use rustc_serialize::{Encodable, Decodable};
use toml::{self, Encoder, Value};

use core::{Resolve, resolver, Workspace};
use core::resolver::WorkspaceResolve;
use util::{CargoResult, ChainError, human, Filesystem};
use util::toml as cargo_toml;

pub fn load_pkg_lockfile(ws: &Workspace) -> CargoResult<Option<Resolve>> {
    if !ws.root().join("Cargo.lock").exists() {
        return Ok(None)
    }

    let root = Filesystem::new(ws.root().to_path_buf());
    let mut f = try!(root.open_ro("Cargo.lock", ws.config(), "Cargo.lock file"));

    let mut s = String::new();
    try!(f.read_to_string(&mut s).chain_error(|| {
        human(format!("failed to read file: {}", f.path().display()))
    }));

    (|| {
        let table = try!(cargo_toml::parse(&s, f.path(), ws.config()));
        let table = toml::Value::Table(table);
        let mut d = toml::Decoder::new(table);
        let v: resolver::EncodableResolve = try!(Decodable::decode(&mut d));
        Ok(Some(try!(v.to_resolve(ws))))
    }).chain_error(|| {
        human(format!("failed to parse lock file at: {}", f.path().display()))
    })
}

pub fn write_pkg_lockfile(ws: &Workspace, resolve: &Resolve) -> CargoResult<()> {
    let mut e = Encoder::new();
    WorkspaceResolve {
        ws: ws,
        resolve: resolve,
    }.encode(&mut e).unwrap();

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
            out.push_str(&metadata.to_string());
        }
        None => {}
    }

    let root = Filesystem::new(ws.root().to_path_buf());

    // Load the original lockfile if it exists.
    //
    // If the lockfile contents haven't changed so don't rewrite it. This is
    // helpful on read-only filesystems.
    let orig = root.open_ro("Cargo.lock", ws.config(), "Cargo.lock file");
    let orig = orig.and_then(|mut f| {
        let mut s = String::new();
        try!(f.read_to_string(&mut s));
        Ok(s)
    });
    if let Ok(orig) = orig {
        if has_crlf_line_endings(&orig) {
            out = out.replace("\n", "\r\n");
        }
        if out == orig {
            return Ok(())
        }
    }

    // Ok, if that didn't work just write it out
    root.open_rw("Cargo.lock", ws.config(), "Cargo.lock file").and_then(|mut f| {
        try!(f.file().set_len(0));
        try!(f.write_all(out.as_bytes()));
        Ok(())
    }).chain_error(|| {
        human(format!("failed to write {}",
                      ws.root().join("Cargo.lock").display()))
    })
}

fn has_crlf_line_endings(s: &str) -> bool {
    // Only check the first line.
    if let Some(lf) = s.find('\n') {
        s[..lf].ends_with('\r')
    } else {
        false
    }
}

fn emit_package(dep: &toml::Table, out: &mut String) {
    out.push_str(&format!("name = {}\n", lookup(dep, "name")));
    out.push_str(&format!("version = {}\n", lookup(dep, "version")));

    if dep.contains_key("source") {
        out.push_str(&format!("source = {}\n", lookup(dep, "source")));
    }

    if let Some(ref s) = dep.get("dependencies") {
        let slice = Value::as_slice(*s).unwrap();

        if !slice.is_empty() {
            out.push_str("dependencies = [\n");

            for child in slice.iter() {
                out.push_str(&format!(" {},\n", child));
            }

            out.push_str("]\n");
        }
        out.push_str("\n");
    } else if dep.contains_key("replace") {
        out.push_str(&format!("replace = {}\n\n", lookup(dep, "replace")));
    }
}

fn lookup<'a>(table: &'a toml::Table, key: &str) -> &'a toml::Value {
    table.get(key).expect(&format!("didn't find {}", key))
}
