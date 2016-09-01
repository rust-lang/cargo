use std::collections::HashMap;

use util::CargoResult;
use util::config::Config;
use util::toml::{TomlDependency, DetailedTomlDependency};

fn marshall<S, I>(name_ver: I)
                  -> HashMap<String, TomlDependency>
    where I: Iterator<Item=(S, &'static str)>,
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

mod default {
    use std::collections::HashMap;
    use util::toml::TomlDependency;
    use super::marshall;

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
}

fn get_custom(sort: &'static str, config: &Config)
              -> CargoResult<Option<HashMap<String, TomlDependency>>>
{
    let overrides = try!(config.get_list(&format!(
        "custom-implicit-stdlib-dependencies.{}",
        sort)));

    Ok(overrides.map(|os| marshall(os.val.into_iter().map(|o| (o.0, "^1.0")))))
}

pub fn primary(config: &Config)
               -> CargoResult<HashMap<String, TomlDependency>>
{
    Ok(try!(get_custom("dependencies", config))
       .unwrap_or_else(|| default::primary()))
}

pub fn dev(config: &Config)
           -> CargoResult<HashMap<String, TomlDependency>>
{
    Ok(try!(get_custom("dev-dependencies", config))
       .unwrap_or_else(|| default::dev()))
}

pub fn build(config: &Config)
             -> CargoResult<HashMap<String, TomlDependency>>
{
    Ok(try!(get_custom("build-dependencies", config))
       .unwrap_or_else(|| default::build()))
}
