use std::fmt;
use std::fmt::{Show,Formatter};
use collections::HashMap;
use serialize::{Encoder,Encodable};
use core::{
    Dependency,
    NameVer,
    Package,
    Summary
};
use util::result::CargoResult;

#[deriving(Eq,Clone)]
pub struct Manifest {
    summary: Summary,
    authors: Vec<~str>,
    targets: Vec<Target>,
    target_dir: Path,
}

impl Show for Manifest {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f.buf, "Manifest({}, authors={}, targets={}, target_dir={})", self.summary, self.authors, self.targets, self.target_dir.display())
    }
}

#[deriving(Eq,Clone,Encodable)]
pub struct SerializedManifest {
    summary: Summary,
    authors: Vec<~str>,
    targets: Vec<Target>,
    target_dir: ~str
}

impl<E, S: Encoder<E>> Encodable<S, E> for Manifest {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        SerializedManifest {
            summary: self.summary.clone(),
            authors: self.authors.clone(),
            targets: self.targets.clone(),
            target_dir: self.target_dir.as_str().unwrap().to_owned()
        }.encode(s)
    }
}

#[deriving(Show,Clone,Eq,Encodable)]
pub enum TargetKind {
    LibTarget,
    BinTarget
}

#[deriving(Clone,Eq)]
pub struct Target {
    kind: TargetKind,
    name: ~str,
    path: Path
}

#[deriving(Encodable)]
pub struct SerializedTarget {
    kind: &'static str,
    name: ~str,
    path: ~str
}

impl<E, S: Encoder<E>> Encodable<S, E> for Target {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let kind = match self.kind {
            LibTarget => "lib",
            BinTarget => "bin"
        };

        SerializedTarget {
            kind: kind,
            name: self.name.clone(),
            path: self.path.as_str().unwrap().to_owned()
        }.encode(s)
    }
}

impl Show for Target {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f.buf, "{}(name={}, path={})", self.kind, self.name, self.path.display())
    }
}

impl Manifest {
    pub fn new(summary: &Summary, targets: &[Target], target_dir: &Path) -> Manifest {
        Manifest {
            summary: summary.clone(),
            authors: Vec::new(),
            targets: Vec::from_slice(targets),
            target_dir: target_dir.clone()
        }
    }

    pub fn get_summary<'a>(&'a self) -> &'a Summary {
        &self.summary
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.get_summary().get_name_ver().get_name()
    }

    pub fn get_authors<'a>(&'a self) -> &'a [~str] {
        self.authors.as_slice()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [Dependency] {
        self.get_summary().get_dependencies()
    }

    pub fn get_targets<'a>(&'a self) -> &'a [Target] {
        self.targets.as_slice()
    }

    pub fn get_target_dir<'a>(&'a self) -> &'a Path {
        &self.target_dir
    }
}

impl Target {
    pub fn lib_target(name: &str, path: &Path) -> Target {
        Target {
            kind: LibTarget,
            name: name.to_owned(),
            path: path.clone()
        }
    }

    pub fn bin_target(name: &str, path: &Path) -> Target {
        Target {
            kind: BinTarget,
            name: name.to_owned(),
            path: path.clone()
        }
    }

    pub fn get_path<'a>(&'a self) -> &'a Path {
        &self.path
    }

    pub fn rustc_crate_type(&self) -> &'static str {
        match self.kind {
            LibTarget => "lib",
            BinTarget => "bin"
        }
    }
}

/*
 *
 * ===== Serialized =====
 *
 */

type TomlLibTarget = TomlTarget;
type TomlExecTarget = TomlTarget;

#[deriving(Decodable,Encodable,Eq,Clone,Show)]
pub struct Project {
    pub name: ~str,
    pub version: ~str,
    pub authors: ~[~str]
}

/*
 * TODO: Make all struct fields private
 */

#[deriving(Decodable,Encodable,Eq,Clone)]
pub struct TomlManifest {
    project: Box<Project>,
    lib: Option<~[TomlLibTarget]>,
    bin: Option<~[TomlExecTarget]>,
    dependencies: Option<HashMap<~str, ~str>>
}

impl TomlManifest {
    pub fn to_package(&self, path: &str) -> CargoResult<Package> {
        // TODO: Convert hte argument to take a Path
        let path = Path::new(path);

        // Get targets
        let targets = normalize(&self.lib, &self.bin);
        // Get deps
        let deps = self.dependencies.clone().map(|deps| {
            deps.iter().map(|(k,v)| {
                // This can produce an invalid version, but it's temporary because this needs
                // to be replaced with Dependency, not NameVer
                Dependency::with_namever(&NameVer::new(k.clone(), v.clone()))
            }).collect()
        }).unwrap_or_else(|| vec!());

        // TODO: https://github.com/mozilla/rust/issues/14049
        let root = Path::new(path.dirname());

        Ok(Package::new(
            &Manifest::new(
                &Summary::new(&self.project.to_name_ver(), deps.as_slice()),
                targets.as_slice(),
                &Path::new("target")),
            &root))
    }
}

impl Project {
    fn to_name_ver(&self) -> NameVer {
        NameVer::new(self.name, self.version)
    }
}

#[deriving(Decodable,Encodable,Eq,Clone)]
struct TomlTarget {
    name: ~str,
    path: Option<~str>
}

fn normalize(lib: &Option<~[TomlLibTarget]>, bin: &Option<~[TomlExecTarget]>) -> Vec<Target> {
    fn lib_targets(dst: &mut Vec<Target>, libs: &[TomlLibTarget]) {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| format!("src/{}.rs", l.name));
        dst.push(Target::lib_target(l.name, &Path::new(path)));
    }

    fn bin_targets(dst: &mut Vec<Target>, bins: &[TomlExecTarget], default: |&TomlExecTarget| -> ~str) {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| default(bin));
            dst.push(Target::bin_target(bin.name.clone(), &Path::new(path)));
        }
    }

    let mut ret = Vec::new();

    match (lib, bin) {
        (&Some(ref libs), &Some(ref bins)) => {
            lib_targets(&mut ret, libs.as_slice());
            bin_targets(&mut ret, bins.as_slice(), |bin| format!("src/bin/{}.rs", bin.name));
        },
        (&Some(ref libs), &None) => {
            lib_targets(&mut ret, libs.as_slice());
        },
        (&None, &Some(ref bins)) => {
            bin_targets(&mut ret, bins.as_slice(), |bin| format!("src/{}.rs", bin.name));
        },
        (&None, &None) => ()
    }

    ret
}
