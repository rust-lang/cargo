#![crate_id="cargo-read-manifest"]
#![allow(deprecated_owned_vector)]

extern crate cargo;
extern crate hammer;
extern crate serialize;
extern crate toml;

use hammer::FlagConfig;
use serialize::Decoder;
use toml::from_toml;
use cargo::{Manifest,LibTarget,ExecTarget,Project,CargoResult,ToCargoError,execute_main_without_stdin};
use std::path::Path;

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
struct SerializedManifest {
    project: ~Project,
    lib: Option<~[SerializedLibTarget]>,
    bin: Option<~[SerializedExecTarget]>
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct SerializedTarget {
    name: ~str,
    path: Option<~str>
}

pub type SerializedLibTarget = SerializedTarget;
pub type SerializedExecTarget = SerializedTarget;


#[deriving(Decodable,Eq,Clone,Ord)]
struct ReadManifestFlags {
    manifest_path: ~str
}

impl FlagConfig for ReadManifestFlags {}

fn main() {
    execute_main_without_stdin::<ReadManifestFlags, Manifest>(execute);
}

fn execute(flags: ReadManifestFlags) -> CargoResult<Option<Manifest>> {
    let manifest_path = flags.manifest_path;
    let root = try!(toml::parse_from_file(manifest_path.clone()).to_cargo_error(format!("Couldn't parse Toml file: {}", manifest_path), 1));

    let toml_manifest = try!(from_toml::<SerializedManifest>(root.clone()).to_cargo_error(|e: toml::Error| format!("{}", e), 1));

    let (lib, bin) = normalize(&toml_manifest.lib, &toml_manifest.bin);

    Ok(Some(Manifest {
        root: try!(Path::new(manifest_path.clone()).dirname_str().to_cargo_error(format!("Could not get dirname from {}", manifest_path), 1)).to_owned(),
        project: toml_manifest.project,
        lib: lib,
        bin: bin
    }))
}

fn normalize(lib: &Option<~[SerializedLibTarget]>, bin: &Option<~[SerializedExecTarget]>) -> (~[LibTarget], ~[ExecTarget]) {
    fn lib_targets(libs: &[SerializedLibTarget]) -> ~[LibTarget] {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| format!("src/{}.rs", l.name));
        ~[LibTarget{ path: path, name: l.name.clone() }]
    }

    fn bin_targets(bins: &[SerializedExecTarget], default: |&SerializedExecTarget| -> ~str) -> ~[ExecTarget] {
        bins.iter().map(|bin| {
            let path = bin.path.clone().unwrap_or_else(|| default(bin));
            ExecTarget{ path: path, name: bin.name.clone() }
        }).collect()
    }

    match (lib, bin) {
        (&Some(ref libs), &Some(ref bins)) => {
            (lib_targets(libs.as_slice()), bin_targets(bins.as_slice(), |bin| format!("src/bin/{}.rs", bin.name)))
        },
        (&Some(ref libs), &None) => {
            (lib_targets(libs.as_slice()), ~[])
        },
        (&None, &Some(ref bins)) => {
            (~[], bin_targets(bins.as_slice(), |bin| format!("src/{}.rs", bin.name)))
        },
        (&None, &None) => {
            (~[], ~[])
        }
    }
}
