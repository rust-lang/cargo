use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::OnceLock;

use cargo::core::dependency::DepKind;
use cargo::core::{Dependency, GitReference, PackageId, SourceId, Summary};
use cargo::util::IntoUrl;

pub trait ToDep {
    fn to_dep(self) -> Dependency;
    fn opt(self) -> Dependency;
    fn with(self, features: &[&'static str]) -> Dependency;
    fn with_default(self) -> Dependency;
    fn rename(self, name: &str) -> Dependency;
}

impl ToDep for &'static str {
    fn to_dep(self) -> Dependency {
        Dependency::parse(self, Some("1.0.0"), registry_loc()).unwrap()
    }
    fn opt(self) -> Dependency {
        let mut dep = self.to_dep();
        dep.set_optional(true);
        dep
    }
    fn with(self, features: &[&'static str]) -> Dependency {
        let mut dep = self.to_dep();
        dep.set_default_features(false);
        dep.set_features(features.into_iter().copied());
        dep
    }
    fn with_default(self) -> Dependency {
        let mut dep = self.to_dep();
        dep.set_default_features(true);
        dep
    }
    fn rename(self, name: &str) -> Dependency {
        let mut dep = self.to_dep();
        dep.set_explicit_name_in_toml(name);
        dep
    }
}

impl ToDep for Dependency {
    fn to_dep(self) -> Dependency {
        self
    }
    fn opt(mut self) -> Dependency {
        self.set_optional(true);
        self
    }
    fn with(mut self, features: &[&'static str]) -> Dependency {
        self.set_default_features(false);
        self.set_features(features.into_iter().copied());
        self
    }
    fn with_default(mut self) -> Dependency {
        self.set_default_features(true);
        self
    }
    fn rename(mut self, name: &str) -> Dependency {
        self.set_explicit_name_in_toml(name);
        self
    }
}

pub trait ToPkgId {
    fn to_pkgid(&self) -> PackageId;
}

impl ToPkgId for PackageId {
    fn to_pkgid(&self) -> PackageId {
        *self
    }
}

impl<'a> ToPkgId for &'a str {
    fn to_pkgid(&self) -> PackageId {
        PackageId::try_new(*self, "1.0.0", registry_loc()).unwrap()
    }
}

impl<T: AsRef<str>, U: AsRef<str>> ToPkgId for (T, U) {
    fn to_pkgid(&self) -> PackageId {
        let (name, vers) = self;
        PackageId::try_new(name.as_ref(), vers.as_ref(), registry_loc()).unwrap()
    }
}

#[macro_export]
macro_rules! pkg {
    ($pkgid:expr => [$($deps:expr),* $(,)? ]) => ({
        use $crate::helpers::ToDep;
        let d: Vec<Dependency> = vec![$($deps.to_dep()),*];
        $crate::helpers::pkg_dep($pkgid, d)
    });

    ($pkgid:expr) => ({
        $crate::helpers::pkg($pkgid)
    })
}

fn registry_loc() -> SourceId {
    static EXAMPLE_DOT_COM: OnceLock<SourceId> = OnceLock::new();
    let example_dot = EXAMPLE_DOT_COM.get_or_init(|| {
        SourceId::for_registry(&"https://example.com".into_url().unwrap()).unwrap()
    });
    *example_dot
}

pub fn pkg<T: ToPkgId>(name: T) -> Summary {
    pkg_dep(name, Vec::new())
}

pub fn pkg_dep<T: ToPkgId>(name: T, dep: Vec<Dependency>) -> Summary {
    let pkgid = name.to_pkgid();
    let link = if pkgid.name().ends_with("-sys") {
        Some(pkgid.name())
    } else {
        None
    };
    Summary::new(name.to_pkgid(), dep, &BTreeMap::new(), link, None).unwrap()
}

pub fn pkg_dep_with<T: ToPkgId>(
    name: T,
    dep: Vec<Dependency>,
    features: &[(&'static str, &[&'static str])],
) -> Summary {
    let pkgid = name.to_pkgid();
    let link = if pkgid.name().ends_with("-sys") {
        Some(pkgid.name())
    } else {
        None
    };
    let features = features
        .into_iter()
        .map(|&(name, values)| (name.into(), values.into_iter().map(|&v| v.into()).collect()))
        .collect();
    Summary::new(name.to_pkgid(), dep, &features, link, None).unwrap()
}

pub fn pkg_dep_link<T: ToPkgId>(name: T, link: &str, dep: Vec<Dependency>) -> Summary {
    Summary::new(name.to_pkgid(), dep, &BTreeMap::new(), Some(link), None).unwrap()
}

pub fn pkg_id(name: &str) -> PackageId {
    PackageId::try_new(name, "1.0.0", registry_loc()).unwrap()
}

pub fn pkg_id_source(name: &str, source: &str) -> PackageId {
    PackageId::try_new(
        name,
        "1.0.0",
        SourceId::for_registry(&source.into_url().unwrap()).unwrap(),
    )
    .unwrap()
}

fn pkg_id_loc(name: &str, loc: &str) -> PackageId {
    let remote = loc.into_url();
    let master = GitReference::Branch("master".to_string());
    let source_id = SourceId::for_git(&remote.unwrap(), master).unwrap();

    PackageId::try_new(name, "1.0.0", source_id).unwrap()
}

pub fn pkg_loc(name: &str, loc: &str) -> Summary {
    let link = if name.ends_with("-sys") {
        Some(name)
    } else {
        None
    };
    Summary::new(
        pkg_id_loc(name, loc),
        Vec::new(),
        &BTreeMap::new(),
        link,
        None,
    )
    .unwrap()
}

pub fn remove_dep(sum: &Summary, ind: usize) -> Summary {
    let mut deps = sum.dependencies().to_vec();
    deps.remove(ind);
    // note: more things will need to be copied over in the future, but it works for now.
    Summary::new(sum.package_id(), deps, &BTreeMap::new(), sum.links(), None).unwrap()
}

pub fn dep(name: &str) -> Dependency {
    dep_req(name, "*")
}

pub fn dep_req(name: &str, req: &str) -> Dependency {
    Dependency::parse(name, Some(req), registry_loc()).unwrap()
}

pub fn dep_req_kind(name: &str, req: &str, kind: DepKind) -> Dependency {
    let mut dep = dep_req(name, req);
    dep.set_kind(kind);
    dep
}

pub fn dep_req_platform(name: &str, req: &str, platform: &str) -> Dependency {
    let mut dep = dep_req(name, req);
    dep.set_platform(Some(platform.parse().unwrap()));
    dep
}

pub fn dep_loc(name: &str, location: &str) -> Dependency {
    let url = location.into_url().unwrap();
    let master = GitReference::Branch("master".to_string());
    let source_id = SourceId::for_git(&url, master).unwrap();
    Dependency::parse(name, Some("1.0.0"), source_id).unwrap()
}

pub fn dep_kind(name: &str, kind: DepKind) -> Dependency {
    let mut dep = dep(name);
    dep.set_kind(kind);
    dep
}

pub fn dep_platform(name: &str, platform: &str) -> Dependency {
    let mut dep = dep(name);
    dep.set_platform(Some(platform.parse().unwrap()));
    dep
}

pub fn registry(pkgs: Vec<Summary>) -> Vec<Summary> {
    pkgs
}

pub fn names<P: ToPkgId>(names: &[P]) -> Vec<PackageId> {
    names.iter().map(|name| name.to_pkgid()).collect()
}

pub fn loc_names(names: &[(&'static str, &'static str)]) -> Vec<PackageId> {
    names
        .iter()
        .map(|&(name, loc)| pkg_id_loc(name, loc))
        .collect()
}

/// Assert `xs` contains `elems`
#[track_caller]
pub fn assert_contains<A: PartialEq + Debug>(xs: &[A], elems: &[A]) {
    for elem in elems {
        assert!(
            xs.contains(elem),
            "missing element\nset: {xs:?}\nmissing: {elem:?}"
        );
    }
}

#[track_caller]
pub fn assert_same<A: PartialEq + Debug>(a: &[A], b: &[A]) {
    assert_eq!(a.len(), b.len(), "not equal\n{a:?}\n{b:?}");
    assert_contains(b, a);
}
