/// Make a path to a dependency, which aligns to
///
/// - [index from of Cargo's index on filesystem][1], and
/// - [index from Crates.io][2].
///
/// <div class="warning">
///
/// Note: For index files, `dep_name` must have had `to_lowercase` called on it.
///
/// </div>
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

pub fn crate_url(dl_template: &str, pkg_name: &str, pkg_version: &str, checksum: &str) -> String {
    use std::fmt::Write as _;

    let mut url = dl_template.to_owned();
    if !url.contains(CRATE_TEMPLATE)
        && !url.contains(VERSION_TEMPLATE)
        && !url.contains(PREFIX_TEMPLATE)
        && !url.contains(LOWER_PREFIX_TEMPLATE)
        && !url.contains(CHECKSUM_TEMPLATE)
    {
        // Original format before customizing the download URL was supported.
        write!(url, "/{}/{}/download", pkg_name, pkg_version).unwrap();
    } else {
        let prefix = make_dep_path(pkg_name, true);
        url = url
            .replace(CRATE_TEMPLATE, pkg_name)
            .replace(VERSION_TEMPLATE, pkg_version)
            .replace(PREFIX_TEMPLATE, &prefix)
            .replace(LOWER_PREFIX_TEMPLATE, &prefix.to_lowercase())
            .replace(CHECKSUM_TEMPLATE, checksum);
    }

    url
}

const CRATE_TEMPLATE: &str = "{crate}";
const VERSION_TEMPLATE: &str = "{version}";
const PREFIX_TEMPLATE: &str = "{prefix}";
const LOWER_PREFIX_TEMPLATE: &str = "{lowerprefix}";
const CHECKSUM_TEMPLATE: &str = "{sha256-checksum}";

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
