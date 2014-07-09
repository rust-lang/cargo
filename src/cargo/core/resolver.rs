use std::collections::HashMap;

use core::{
    Dependency,
    PackageId,
    Summary,
    Registry,
};

use util::{CargoResult, human, internal};

/* TODO:
 * - The correct input here is not a registry. Resolves should be performable
 * on package summaries vs. the packages themselves.
 */
pub fn resolve<R: Registry>(deps: &[Dependency],
                            registry: &mut R) -> CargoResult<Vec<PackageId>> {
    log!(5, "resolve; deps={}", deps);

    let mut remaining = Vec::from_slice(deps);
    let mut resolve = HashMap::<String, Summary>::new();

    loop {
        let curr = match remaining.pop() {
            Some(curr) => curr,
            None => {
                let ret = resolve.values().map(|summary| {
                    summary.get_package_id().clone()
                }).collect();
                log!(5, "resolve complete; ret={}", ret);
                return Ok(ret);
            }
        };

        let opts = try!(registry.query(&curr));

        if opts.len() == 0 {
            return Err(human(format!("No package named {} found", curr.get_name())));
        }

        if opts.len() > 1 {
            return Err(internal(format!("At the moment, Cargo only supports a \
                single source for a particular package name ({}).", curr.get_name())));
        }

        let pkg = opts.get(0).clone();
        resolve.insert(pkg.get_name().to_string(), pkg.clone());

        for dep in pkg.get_dependencies().iter() {
            if !dep.is_transitive() { continue; }

            if !resolve.contains_key_equiv(&dep.get_name()) {
                remaining.push(dep.clone());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use hamcrest::{assert_that, equal_to, contains};

    use core::source::{SourceId, RegistryKind, Location, Remote};
    use core::{Dependency, PackageId, Summary};
    use super::resolve;

    trait ToDep {
        fn to_dep(self) -> Dependency;
    }

    impl ToDep for &'static str {
        fn to_dep(self) -> Dependency {
            let url = from_str("http://example.com").unwrap();
            let source_id = SourceId::new(RegistryKind, Remote(url));
            Dependency::parse(self, Some("1.0.0"), &source_id).unwrap()
        }
    }

    impl ToDep for Dependency {
        fn to_dep(self) -> Dependency {
            self
        }
    }

    macro_rules! pkg(
        ($name:expr => $($deps:expr),+) => ({
            let d: Vec<Dependency> = vec!($($deps.to_dep()),+);

            Summary::new(&PackageId::new($name, "1.0.0", &registry_loc()).unwrap(),
                         d.as_slice())
        });

        ($name:expr) => (
            Summary::new(&PackageId::new($name, "1.0.0", &registry_loc()).unwrap(),
                         [])
        )
    )

    fn registry_loc() -> SourceId {
        let remote = Location::parse("http://example.com").unwrap();
        SourceId::new(RegistryKind, remote)
    }

    fn pkg(name: &str) -> Summary {
        Summary::new(&PackageId::new(name, "1.0.0", &registry_loc()).unwrap(),
                     &[])
    }

    fn dep(name: &str) -> Dependency {
        let url = from_str("http://example.com").unwrap();
        let source_id = SourceId::new(RegistryKind, Remote(url));
        Dependency::parse(name, Some("1.0.0"), &source_id).unwrap()
    }

    fn registry(pkgs: Vec<Summary>) -> Vec<Summary> {
        pkgs
    }

    fn names(names: &[&'static str]) -> Vec<PackageId> {
        names.iter()
            .map(|name| PackageId::new(*name, "1.0.0", &registry_loc()).unwrap())
            .collect()
    }

    #[test]
    pub fn test_resolving_empty_dependency_list() {
        let res = resolve([], &mut registry(vec!())).unwrap();

        assert_that(&res, equal_to(&names([])));
    }

    #[test]
    pub fn test_resolving_only_package() {
        let mut reg = registry(vec!(pkg("foo")));
        let res = resolve([dep("foo")], &mut reg);

        assert_that(&res.unwrap(), equal_to(&names(["foo"])));
    }

    #[test]
    pub fn test_resolving_one_dep() {
        let mut reg = registry(vec!(pkg("foo"), pkg("bar")));
        let res = resolve([dep("foo")], &mut reg);

        assert_that(&res.unwrap(), equal_to(&names(["foo"])));
    }

    #[test]
    pub fn test_resolving_multiple_deps() {
        let mut reg = registry(vec!(pkg!("foo"), pkg!("bar"), pkg!("baz")));
        let res = resolve([dep("foo"), dep("baz")], &mut reg).unwrap();

        assert_that(&res, contains(names(["foo", "baz"])).exactly());
    }

    #[test]
    pub fn test_resolving_transitive_deps() {
        let mut reg = registry(vec!(pkg!("foo"), pkg!("bar" => "foo")));
        let res = resolve([dep("bar")], &mut reg).unwrap();

        assert_that(&res, contains(names(["foo", "bar"])));
    }

    #[test]
    pub fn test_resolving_common_transitive_deps() {
        let mut reg = registry(vec!(pkg!("foo" => "bar"), pkg!("bar")));
        let res = resolve([dep("foo"), dep("bar")], &mut reg).unwrap();

        assert_that(&res, contains(names(["foo", "bar"])));
    }

    #[test]
    pub fn test_resolving_with_dev_deps() {
        let mut reg = registry(vec!(
            pkg!("foo" => "bar", dep("baz").as_dev()),
            pkg!("baz" => "bat", dep("bam").as_dev()),
            pkg!("bar"),
            pkg!("bat")
        ));

        let res = resolve([dep("foo"), dep("baz").as_dev()], &mut reg).unwrap();

        assert_that(&res, contains(names(["foo", "bar", "baz"])));
    }
}
