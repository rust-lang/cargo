use std::path::Path;

use url::Url;

use util::{human, CargoResult};

pub trait ToUrl {
    fn to_url(self) -> CargoResult<Url>;
}

impl<'a> ToUrl for &'a str {
    fn to_url(self) -> CargoResult<Url> {
        Url::parse(self).map_err(|s| {
            human(format!("invalid url `{}`: {}", self, s))
        })
    }
}

impl<'a> ToUrl for &'a Path {
    fn to_url(self) -> CargoResult<Url> {
        Url::from_file_path(self).map_err(|()| {
            human(format!("invalid path url `{}`", self.display()))
        })
    }
}
