use toml;
use hammer::FlagConfig;
use serialize::Decoder;
use toml::from_toml;
use {CargoResult,ToCargoError,core};
use std::path::Path;
use collections::HashMap;
use core::NameVer;

#[deriving(Decodable,Encodable,Eq,Clone)]
struct SerializedManifest {
    project: ~core::Project,
    lib: Option<~[SerializedLibTarget]>,
    bin: Option<~[SerializedExecTarget]>,
    dependencies: HashMap<~str, ~str>
}

#[deriving(Decodable,Encodable,Eq,Clone)]
pub struct SerializedTarget {
    name: ~str,
    path: Option<~str>
}

pub type SerializedLibTarget = SerializedTarget;
pub type SerializedExecTarget = SerializedTarget;


#[deriving(Decodable,Eq,Clone,Ord)]
pub struct ReadManifestFlags {
    manifest_path: ~str
}

impl FlagConfig for ReadManifestFlags {}

pub fn read_manifest(manifest_path: &str) -> CargoResult<core::Manifest> {
    match execute(ReadManifestFlags { manifest_path: manifest_path.to_owned() }) {
        Ok(manifest) => Ok(manifest.unwrap()),
        Err(e) => Err(e)
    }
}

pub fn execute(flags: ReadManifestFlags) -> CargoResult<Option<core::Manifest>> {
    let manifest_path = flags.manifest_path;
    let root = try!(toml::parse_from_file(manifest_path.clone()).to_cargo_error(format!("Couldn't parse Toml file: {}", manifest_path), 1));

    let toml_manifest = try!(from_toml::<SerializedManifest>(root.clone()).to_cargo_error(|e: toml::Error| format!("Couldn't parse Toml file: {:?}", e), 1));

    let (lib, bin) = normalize(&toml_manifest.lib, &toml_manifest.bin);

    let SerializedManifest { project, dependencies, .. } = toml_manifest;

    Ok(Some(core::Manifest {
        root: try!(Path::new(manifest_path.clone()).dirname_str().to_cargo_error(format!("Could not get dirname from {}", manifest_path), 1)).to_owned(),
        project: project,
        lib: lib,
        bin: bin,
        dependencies: dependencies.iter().map(|(k,v)| NameVer::new(k.clone(),v.clone())).collect()
    }))
}

fn normalize(lib: &Option<~[SerializedLibTarget]>, bin: &Option<~[SerializedExecTarget]>) -> (~[core::LibTarget], ~[core::ExecTarget]) {
    fn lib_targets(libs: &[SerializedLibTarget]) -> ~[core::LibTarget] {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| format!("src/{}.rs", l.name));
        ~[core::LibTarget { path: path, name: l.name.clone() }]
    }

    fn bin_targets(bins: &[SerializedExecTarget], default: |&SerializedExecTarget| -> ~str) -> ~[core::ExecTarget] {
        bins.iter().map(|bin| {
            let path = bin.path.clone().unwrap_or_else(|| default(bin));
            core::ExecTarget { path: path, name: bin.name.clone() }
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
