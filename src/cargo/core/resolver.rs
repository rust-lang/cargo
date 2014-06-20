use std::collections::HashMap;

use core::{
    Dependency,
    PackageId,
    Summary,
    Registry
};
use util::errors::CargoResult;

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

        assert!(opts.len() > 0, "no matches for {}", curr.get_name());
        // Temporary, but we must have exactly one option to satisfy the dep
        assert!(opts.len() == 1, "invalid num of results {}", opts.len());

        let pkg = opts.get(0).clone();
        resolve.insert(pkg.get_name().to_str(), pkg.clone());

        for dep in pkg.get_dependencies().iter() {
            if !resolve.contains_key_equiv(&dep.get_name()) {
                remaining.push(dep.clone());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use url;

    use hamcrest::{
        assert_that,
        equal_to,
        contains
    };

    use core::source::{
        SourceId,
        RegistryKind
    };

    use core::{
        Dependency,
        PackageId,
        Summary
    };

    use super::{
        resolve
    };

    macro_rules! pkg(
        ($name:expr => $($deps:expr),+) => (
            {
            let url = url::from_str("http://example.com").unwrap();
            let source_id = SourceId::new(RegistryKind, url);
            let d: Vec<Dependency> = vec!($($deps),+).iter().map(|s| {
                Dependency::parse(*s, "1.0.0", &source_id).unwrap()
            }).collect();
            Summary::new(&PackageId::new($name, "1.0.0",
                                         "http://www.example.com/"),
                         d.as_slice())
            }
        );

        ($name:expr) => (
            Summary::new(&PackageId::new($name, "1.0.0",
                                         "http://www.example.com/"), [])
        )
    )

    fn pkg(name: &str) -> Summary {
        Summary::new(&PackageId::new(name, "1.0.0", "http://www.example.com/"),
                     &[])
    }

    fn dep(name: &str) -> Dependency {
        let url = url::from_str("http://example.com").unwrap();
        let source_id = SourceId::new(RegistryKind, url);
        Dependency::parse(name, "1.0.0", &source_id).unwrap()
    }

    fn registry(pkgs: Vec<Summary>) -> Vec<Summary> {
        pkgs
    }

    fn names(names: &[&'static str]) -> Vec<PackageId> {
        names.iter()
            .map(|name| PackageId::new(*name, "1.0.0", "http://www.example.com/"))
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
}
