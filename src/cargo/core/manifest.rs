use std::fmt;
use std::fmt::{Show,Formatter};
use collections::HashMap;
use semver::Version;
use serialize::{Encoder,Encodable};
use core::{
    Dependency,
    NameVer,
    Package,
    Summary
};
use core::dependency::SerializedDependency;
use util::CargoResult;

#[deriving(Eq,Clone)]
pub struct Manifest {
    summary: Summary,
    authors: Vec<StrBuf>,
    targets: Vec<Target>,
    target_dir: Path,
}

impl Show for Manifest {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Manifest({}, authors={}, targets={}, target_dir={})", self.summary, self.authors, self.targets, self.target_dir.display())
    }
}

#[deriving(Eq,Clone,Encodable)]
pub struct SerializedManifest {
    name: StrBuf,
    version: StrBuf,
    dependencies: Vec<SerializedDependency>,
    authors: Vec<StrBuf>,
    targets: Vec<Target>,
    target_dir: StrBuf
}

impl<E, S: Encoder<E>> Encodable<S, E> for Manifest {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        SerializedManifest {
            name: self.summary.get_name().to_strbuf(),
            version: format_strbuf!("{}", self.summary.get_version()),
            dependencies: self.summary.get_dependencies().iter().map(|d| SerializedDependency::from_dependency(d)).collect(),
            authors: self.authors.clone(),
            targets: self.targets.clone(),
            target_dir: self.target_dir.as_str().unwrap().to_strbuf()
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
    name: StrBuf,
    path: Path
}

#[deriving(Encodable)]
pub struct SerializedTarget {
    kind: &'static str,
    name: StrBuf,
    path: StrBuf
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
            path: self.path.as_str().unwrap().to_strbuf()
        }.encode(s)
    }
}

impl Show for Target {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}(name={}, path={})", self.kind, self.name, self.path.display())
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

    pub fn get_version<'a>(&'a self) -> &'a Version {
        self.get_summary().get_name_ver().get_version()
    }

    pub fn get_authors<'a>(&'a self) -> &'a [StrBuf] {
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
            name: name.to_strbuf(),
            path: path.clone()
        }
    }

    pub fn bin_target(name: &str, path: &Path) -> Target {
        Target {
            kind: BinTarget,
            name: name.to_strbuf(),
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
    pub name: StrBuf,
    pub version: StrBuf,
    pub authors: Vec<StrBuf>
}

/*
 * TODO: Make all struct fields private
 */

#[deriving(Decodable,Encodable,Eq,Clone)]
pub struct TomlManifest {
    project: Box<Project>,
    lib: Option<~[TomlLibTarget]>,
    bin: Option<~[TomlExecTarget]>,
    dependencies: Option<HashMap<StrBuf, StrBuf>>
}

impl TomlManifest {
    pub fn to_package(&self, path: &str) -> CargoResult<Package> {
        // TODO: Convert hte argument to take a Path
        let path = Path::new(path);

        // Get targets
        let targets = normalize(&self.lib, &self.bin);

        let mut deps = Vec::new();

        // Collect the deps
        match self.dependencies {
            Some(ref dependencies) => {
                for (n, v) in dependencies.iter() {
                    deps.push(try!(Dependency::parse(n.as_slice(), v.as_slice())));
                }
            }
            None => ()
        }

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
        NameVer::new(self.name.as_slice(), self.version.as_slice())
    }
}

#[deriving(Decodable,Encodable,Eq,Clone)]
struct TomlTarget {
    name: StrBuf,
    path: Option<StrBuf>
}

fn normalize(lib: &Option<~[TomlLibTarget]>, bin: &Option<~[TomlExecTarget]>) -> Vec<Target> {
    fn lib_targets(dst: &mut Vec<Target>, libs: &[TomlLibTarget]) {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| format_strbuf!("src/{}.rs", l.name));
        dst.push(Target::lib_target(l.name.as_slice(), &Path::new(path)));
    }

    fn bin_targets(dst: &mut Vec<Target>, bins: &[TomlExecTarget], default: |&TomlExecTarget| -> StrBuf) {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| default(bin));
            dst.push(Target::bin_target(bin.name.as_slice(), &Path::new(path)));
        }
    }

    let mut ret = Vec::new();

    match (lib, bin) {
        (&Some(ref libs), &Some(ref bins)) => {
            lib_targets(&mut ret, libs.as_slice());
            bin_targets(&mut ret, bins.as_slice(), |bin| format_strbuf!("src/bin/{}.rs", bin.name));
        },
        (&Some(ref libs), &None) => {
            lib_targets(&mut ret, libs.as_slice());
        },
        (&None, &Some(ref bins)) => {
            bin_targets(&mut ret, bins.as_slice(), |bin| format_strbuf!("src/{}.rs", bin.name));
        },
        (&None, &None) => ()
    }

    ret
}
