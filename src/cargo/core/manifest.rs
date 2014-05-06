use core::NameVer;
use core::dependency::Dependency;
use collections::HashMap;
use core::errors::{CargoResult,CargoError,ToResult,PathError};

/*
 * TODO: Make all struct fields private
 */

#[deriving(Decodable,Encodable,Eq,Clone)]
pub struct SerializedManifest {
    project: ~Project,
    lib: Option<~[SerializedLibTarget]>,
    bin: Option<~[SerializedExecTarget]>,
    dependencies: Option<HashMap<~str, ~str>>
}

#[deriving(Decodable,Encodable,Eq,Clone)]
struct SerializedTarget {
    name: ~str,
    path: Option<~str>
}

type SerializedLibTarget = SerializedTarget;
type SerializedExecTarget = SerializedTarget;

#[deriving(Decodable,Encodable,Eq,Clone,Show)]
pub struct Manifest {
    pub project: ~Project,
    pub root: ~str,
    pub lib: ~[LibTarget],
    pub bin: ~[ExecTarget],
    pub target: ~str,
    pub dependencies: Vec<Dependency>
}

impl Manifest {
    pub fn from_serialized(path: &str, serialized: &SerializedManifest) -> CargoResult<Manifest> {
        let (lib,bin) = normalize(&serialized.lib, &serialized.bin);
        let &SerializedManifest { ref project, ref dependencies, .. } = serialized;

        let deps = dependencies.clone().map(|deps| {
            deps.iter().map(|(k,v)| {
                // This can produce an invalid version, but it's temporary because this needs
                // to be replaced with Dependency, not NameVer
                Dependency::with_namever(&NameVer::new(k.clone(), v.clone()))
            }).collect()
        }).unwrap_or_else(|| vec!());

        let root = try!(Path::new(path.to_owned()).dirname_str().map(|s| s.to_owned()).to_result(|_|
            CargoError::internal(PathError(format!("Couldn't convert {} to a directory name", path)))));

        Ok(Manifest {
            root: root.to_owned(),
            project: project.clone(),
            lib: lib,
            bin: bin,
            target: "target".to_owned(),
            dependencies: deps
        })
    }

    pub fn get_name_ver(&self) -> NameVer {
        NameVer::new(self.project.name.as_slice(), self.project.version.as_slice())
    }

    pub fn get_path<'a>(&'a self) -> Path {
        Path::new(self.root.as_slice())
    }
}

fn normalize(lib: &Option<~[SerializedLibTarget]>, bin: &Option<~[SerializedExecTarget]>) -> (~[LibTarget], ~[ExecTarget]) {
    fn lib_targets(libs: &[SerializedLibTarget]) -> ~[LibTarget] {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| format!("src/{}.rs", l.name));
        ~[LibTarget { path: path, name: l.name.clone() }]
    }

    fn bin_targets(bins: &[SerializedExecTarget], default: |&SerializedExecTarget| -> ~str) -> ~[ExecTarget] {
        bins.iter().map(|bin| {
            let path = bin.path.clone().unwrap_or_else(|| default(bin));
            ExecTarget { path: path, name: bin.name.clone() }
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

#[deriving(Decodable,Encodable,Eq,Clone,Show)]
pub struct ExecTarget {
    pub name: ~str,
    pub path: ~str
}

#[deriving(Decodable,Encodable,Eq,Clone,Show)]
pub struct LibTarget {
    pub name: ~str,
    pub path: ~str
}

#[deriving(Decodable,Encodable,Eq,Clone,Show)]
pub struct Project {
    pub name: ~str,
    pub version: ~str,
    pub authors: ~[~str]
}
