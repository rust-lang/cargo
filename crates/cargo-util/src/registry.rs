/// Make a path to a dependency, which aligns to
///
/// - [index from of Cargo's index on filesystem][1], and
/// - [index from Crates.io][2].
///
/// [1]: https://docs.rs/cargo/latest/cargo/sources/registry/index.html#the-format-of-the-index
/// [2]: https://github.com/rust-lang/crates.io-index
pub fn make_dep_path(dep_name: &str, prefix_only: bool) -> String {
    let (slash, name) = if prefix_only {
        ("", "")
    } else {
        ("/", dep_name)
    };
    match dep_name.len() {
        1 => format!("1{}{}", slash, name),
        2 => format!("2{}{}", slash, name),
        3 => format!("3/{}{}{}", &dep_name[..1], slash, name),
        _ => format!("{}/{}{}{}", &dep_name[0..2], &dep_name[2..4], slash, name),
    }
}

#[cfg(test)]
mod tests {
    use super::make_dep_path;

    #[test]
    fn prefix_only() {
        assert_eq!(make_dep_path("a", true), "1");
        assert_eq!(make_dep_path("ab", true), "2");
        assert_eq!(make_dep_path("abc", true), "3/a");
        assert_eq!(make_dep_path("Abc", true), "3/A");
        assert_eq!(make_dep_path("AbCd", true), "Ab/Cd");
        assert_eq!(make_dep_path("aBcDe", true), "aB/cD");
    }

    #[test]
    fn full() {
        assert_eq!(make_dep_path("a", false), "1/a");
        assert_eq!(make_dep_path("ab", false), "2/ab");
        assert_eq!(make_dep_path("abc", false), "3/a/abc");
        assert_eq!(make_dep_path("Abc", false), "3/A/Abc");
        assert_eq!(make_dep_path("AbCd", false), "Ab/Cd/AbCd");
        assert_eq!(make_dep_path("aBcDe", false), "aB/cD/aBcDe");
    }
}
