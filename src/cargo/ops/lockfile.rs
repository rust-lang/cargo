use std::io::prelude::*;

use toml;

use core::{Resolve, resolver, Workspace};
use core::resolver::WorkspaceResolve;
use util::{human, Filesystem};
use util::errors::{CargoResult, CargoResultExt};
use util::toml as cargo_toml;

pub fn load_pkg_lockfile(ws: &Workspace) -> CargoResult<Option<Resolve>> {
    if !ws.root().join("Cargo.lock").exists() {
        return Ok(None)
    }

    let root = Filesystem::new(ws.root().to_path_buf());
    let mut f = root.open_ro("Cargo.lock", ws.config(), "Cargo.lock file")?;

    let mut s = String::new();
    f.read_to_string(&mut s).chain_err(|| {
        human(format!("failed to read file: {}", f.path().display()))
    })?;

    (|| -> CargoResult<Option<Resolve>> {
        let resolve : toml::Value = cargo_toml::parse(&s, f.path(), ws.config())?;
        let v: resolver::EncodableResolve = resolve.try_into()?;
        Ok(Some(v.into_resolve(ws)?))
    })().chain_err(|| {
        human(format!("failed to parse lock file at: {}", f.path().display()))
    })
}

pub fn write_pkg_lockfile(ws: &Workspace, resolve: &Resolve) -> CargoResult<()> {
    // Load the original lockfile if it exists.
    let ws_root = Filesystem::new(ws.root().to_path_buf());
    let orig = ws_root.open_ro("Cargo.lock", ws.config(), "Cargo.lock file");
    let orig = orig.and_then(|mut f| {
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        Ok(s)
    });

    // Forward compatibility: if `orig` uses rootless format
    // from the future, do the same.
    let use_root_key = if let Ok(ref orig) = orig {
        !orig.starts_with("[[package]]")
    } else {
        true
    };

    let toml = toml::Value::try_from(WorkspaceResolve {
        ws: ws,
        resolve: resolve,
        use_root_key: use_root_key,
    }).unwrap();

    let mut out = String::new();

    // Note that we do not use e.toml.to_string() as we want to control the
    // exact format the toml is in to ensure pretty diffs between updates to the
    // lockfile.
    if let Some(root) = toml.get("root") {
        out.push_str("[root]\n");
        emit_package(root.as_table().unwrap(), &mut out);
    }

    let deps = toml["package"].as_array().unwrap();
    for dep in deps.iter() {
        let dep = dep.as_table().unwrap();

        out.push_str("[[package]]\n");
        emit_package(dep, &mut out);
    }

    if let Some(meta) = toml.get("metadata") {
        out.push_str("[metadata]\n");
        out.push_str(&meta.to_string());
    }

    // If the lockfile contents haven't changed so don't rewrite it. This is
    // helpful on read-only filesystems.
    if let Ok(orig) = orig {
        if has_crlf_line_endings(&orig) {
            out = out.replace("\n", "\r\n");
        }
        if out == orig {
            return Ok(())
        }
    }

    if !ws.config().lock_update_allowed() {
        let flag = if ws.config().network_allowed() {"--frozen"} else {"--locked"};
        bail!("the lock file needs to be updated but {} was passed to \
               prevent this", flag);
    }

    // Ok, if that didn't work just write it out
    ws_root.open_rw("Cargo.lock", ws.config(), "Cargo.lock file").and_then(|mut f| {
        f.file().set_len(0)?;
        f.write_all(out.as_bytes())?;
        Ok(())
    }).chain_err(|| {
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

fn emit_package(dep: &toml::value::Table, out: &mut String) {
    out.push_str(&format!("name = {}\n", &dep["name"]));
    out.push_str(&format!("version = {}\n", &dep["version"]));

    if dep.contains_key("source") {
        out.push_str(&format!("source = {}\n", &dep["source"]));
    }

    if let Some(ref s) = dep.get("dependencies") {
        let slice = s.as_array().unwrap();

        if !slice.is_empty() {
            out.push_str("dependencies = [\n");

            for child in slice.iter() {
                out.push_str(&format!(" {},\n", child));
            }

            out.push_str("]\n");
        }
        out.push_str("\n");
    } else if dep.contains_key("replace") {
        out.push_str(&format!("replace = {}\n\n", &dep["replace"]));
    }
}
