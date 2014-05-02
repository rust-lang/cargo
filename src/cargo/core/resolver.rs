use collections::HashMap;
use core;
use core::package::PackageSet;
use {CargoResult};

#[allow(dead_code)]
pub fn resolve(deps: &[core::Dependency], registry: &core::Registry) -> CargoResult<PackageSet> {
    let mut remaining = Vec::from_slice(deps);
    let mut resolve = HashMap::<&str, &core::Package>::new();

    loop {
        let curr = match remaining.pop() {
            Some(curr) => curr,
            None => {
                let packages: Vec<core::Package> = resolve.values().map(|v| (*v).clone()).collect();
                return Ok(PackageSet::new(packages.as_slice()))
            }
        };

        let opts = registry.query(curr.get_name());

        //assert!(!resolve.contains_key_equiv(&curr.get_name()), "already traversed {}", curr.get_name());
        // Temporary, but we must have exactly one option to satisfy the dep
        assert!(opts.len() == 1, "invalid num of results {}", opts.len());

        let pkg = opts.get(0);
        resolve.insert(pkg.get_name(), *pkg);

        for dep in pkg.get_dependencies().iter() {
            if !resolve.contains_key_equiv(&dep.get_name()) {
                remaining.push(dep.clone());
            }
        }
    }
}

#[cfg(test)]
mod test {

    use hamcrest::{
        assert_that,
        equal_to,
        contains
    };

    use core::{
        MemRegistry,
        Dependency,
        Package
    };

    use super::{
        resolve
    };

    macro_rules! pkg(
        ($name:expr => $($deps:expr),+) => (
            Package::new($name, &vec!($($deps),+).iter().map(|s| Dependency::new(*s)).collect())
        );

        ($name:expr) => (
            Package::new($name, &vec!())
        )
    )

    fn pkg(name: &str) -> Package {
        Package::new(name, &Vec::<Dependency>::new())
    }

    fn dep(name: &str) -> Dependency {
        Dependency::new(name)
    }

    fn registry(pkgs: Vec<Package>) -> MemRegistry {
        MemRegistry::new(&pkgs)
    }

    #[test]
    pub fn test_resolving_empty_dependency_list() {
        let res = resolve(&vec!(), &registry(vec!())).unwrap();

        assert_that(&res, equal_to(&Vec::<Package>::new()));
    }

    #[test]
    pub fn test_resolving_only_package() {
        let reg = registry(vec!(pkg("foo")));
        let res = resolve(&vec!(dep("foo")), &reg);

        assert_that(&res.unwrap(), equal_to(&vec!(pkg("foo"))));
    }

    #[test]
    pub fn test_resolving_one_dep() {
        let reg = registry(vec!(pkg("foo"), pkg("bar")));
        let res = resolve(&vec!(dep("foo")), &reg);

        assert_that(&res.unwrap(), equal_to(&vec!(pkg("foo"))));
    }

    #[test]
    pub fn test_resolving_multiple_deps() {
        let reg = registry(vec!(pkg!("foo"), pkg!("bar"), pkg!("baz")));
        let res = resolve(&vec!(dep("foo"), dep("baz")), &reg).unwrap();

        assert_that(&res, contains(vec!(pkg("foo"), pkg("baz"))).exactly());
    }

    #[test]
    pub fn test_resolving_transitive_deps() {
        let reg = registry(vec!(pkg!("foo"), pkg!("bar" => "foo")));
        let res = resolve(&vec!(dep("bar")), &reg).unwrap();

        assert_that(&res, contains(vec!(pkg!("foo"), pkg!("bar" => "foo"))));
    }

    #[test]
    pub fn test_resolving_common_transitive_deps() {
        let reg = registry(vec!(pkg!("foo" => "bar"), pkg!("bar")));
        let res = resolve(&vec!(dep("foo"), dep("bar")), &reg).unwrap();

        assert_that(&res, contains(vec!(pkg!("foo" => "bar"), pkg!("bar"))));
    }
}
