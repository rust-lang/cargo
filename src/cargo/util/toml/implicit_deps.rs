use std::collections::HashMap;

use util::toml::{TomlDependency, DetailedTomlDependency};


fn marshall<S, I>(name_ver: I)
                  -> HashMap<String, TomlDependency>
    where I: Iterator<Item=(S, S)>,
          S: Into<String>
{
    name_ver
        .map(|(n, v)| (n.into(),
                       TomlDependency::Detailed(DetailedTomlDependency {
                           version: Some(v.into()),
                           stdlib: Some(true),
                           .. Default::default()
                       })))
        .collect()
}

pub fn primary() -> HashMap<String, TomlDependency> {
    marshall(vec![
        ("core", "^1.0"),
        ("std", "^1.0"),
    ].into_iter())
}

pub fn dev() -> HashMap<String, TomlDependency> {
    let mut map = marshall(vec![
        ("test", "^1.0"),
    ].into_iter());
    map.extend(self::primary().into_iter());
    map
}

pub fn build() -> HashMap<String, TomlDependency> {
    self::primary()
}
