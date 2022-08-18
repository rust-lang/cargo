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

    let mut chars = [None; 4];
    dep_name.chars().enumerate().try_for_each(|(i, c)| {
        *chars.get_mut(i)? = Some(c);
        Some(())
    });

    match chars {
        [None, ..] => panic!("length of a crate name must not be zero"),
        [Some(_), None, ..] => format!("1{slash}{name}"),
        [Some(_), Some(_), None, ..] => format!("2{slash}{name}"),
        [Some(f0), Some(_), Some(_), None] => {
            format!("3/{f0}{slash}{name}")
        }
        [Some(f0), Some(f1), Some(s0), Some(s1)] => {
            format!("{f0}{f1}/{s0}{s1}{slash}{name}")
        }
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

    // This test checks that non-ASCII strings are handled correctly in 2 cases
    // - 3 byte string, where byte index 2 isn't a char boundary
    // - at least 4 byte string, where byte index 4 isn't a char boundary
    #[test]
    fn test_10993() {
        assert_eq!(make_dep_path("ĉa", true), "2");
        assert_eq!(make_dep_path("abcĉ", true), "ab/cĉ");

        assert_eq!(make_dep_path("ĉa", false), "2/ĉa");
        assert_eq!(make_dep_path("abcĉ", false), "ab/cĉ/abcĉ");
    }
}
