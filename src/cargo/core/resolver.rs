use collections::HashMap;
use core::{
    Dependency,
    NameVer,
    Summary,
    Registry
};
use util::result::CargoResult;

/* TODO:
 * - The correct input here is not a registry. Resolves should be performable
 * on package summaries vs. the packages themselves.
 */
#[allow(dead_code)]
pub fn resolve(deps: &[Dependency], registry: &Registry) -> CargoResult<Vec<NameVer>> {
    let mut remaining = Vec::from_slice(deps);
    let mut resolve = HashMap::<&str, &Summary>::new();

    loop {
        let curr = match remaining.pop() {
            Some(curr) => curr,
            None => {
                return Ok(resolve.values().map(|summary| summary.get_name_ver().clone()).collect());
            }
        };

        let opts = registry.query(curr.get_name());

        assert!(opts.len() > 0, "no matches for {}", curr.get_name());
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
        Dependency,
        NameVer,
        Summary
    };

    use super::{
        resolve
    };

    macro_rules! pkg(
        ($name:expr => $($deps:expr),+) => (
            {
            let d: Vec<Dependency> = vec!($($deps),+).iter().map(|s| Dependency::parse(*s, "1.0.0").unwrap()).collect();
            Summary::new(&NameVer::new($name, "1.0.0"), d.as_slice())
            }
        );

        ($name:expr) => (
            Summary::new(&NameVer::new($name, "1.0.0"), [])
        )
    )

    fn pkg(name: &str) -> Summary {
        Summary::new(&NameVer::new(name, "1.0.0"), &[])
    }

    fn dep(name: &str) -> Dependency {
        Dependency::parse(name, "1.0.0").unwrap()
    }

    fn registry(pkgs: Vec<Summary>) -> Vec<Summary> {
        pkgs
    }

    fn names(names: &[&'static str]) -> Vec<NameVer> {
        names.iter()
            .map(|name| NameVer::new(*name, "1.0.0"))
            .collect()
    }

    #[test]
    pub fn test_resolving_empty_dependency_list() {
        let res = resolve([], &registry(vec!())).unwrap();

        assert_that(&res, equal_to(&names([])));
    }

    #[test]
    pub fn test_resolving_only_package() {
        let reg = registry(vec!(pkg("foo")));
        let res = resolve([dep("foo")], &reg);

        assert_that(&res.unwrap(), equal_to(&names(["foo"])));
    }

    #[test]
    pub fn test_resolving_one_dep() {
        let reg = registry(vec!(pkg("foo"), pkg("bar")));
        let res = resolve([dep("foo")], &reg);

        assert_that(&res.unwrap(), equal_to(&names(["foo"])));
    }

    #[test]
    pub fn test_resolving_multiple_deps() {
        let reg = registry(vec!(pkg!("foo"), pkg!("bar"), pkg!("baz")));
        let res = resolve([dep("foo"), dep("baz")], &reg).unwrap();

        assert_that(&res, contains(names(["foo", "baz"])).exactly());
    }

    #[test]
    pub fn test_resolving_transitive_deps() {
        let reg = registry(vec!(pkg!("foo"), pkg!("bar" => "foo")));
        let res = resolve([dep("bar")], &reg).unwrap();

        assert_that(&res, contains(names(["foo", "bar"])));
    }

    #[test]
    pub fn test_resolving_common_transitive_deps() {
        let reg = registry(vec!(pkg!("foo" => "bar"), pkg!("bar")));
        let res = resolve([dep("foo"), dep("bar")], &reg).unwrap();

        assert_that(&res, contains(names(["foo", "bar"])));
    }
}
