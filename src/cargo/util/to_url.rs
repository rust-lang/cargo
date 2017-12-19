use std::path::Path;

use url::Url;

use util::CargoResult;

/// A type that can be converted to a Url
pub trait ToUrl {
    /// Performs the conversion
    fn to_url(self) -> CargoResult<Url>;
}

impl<'a> ToUrl for &'a str {
    fn to_url(self) -> CargoResult<Url> {
        Url::parse(self).map_err(|s| {
            format_err!("invalid url `{}`: {}", self, s)
        })
    }
}

impl<'a> ToUrl for &'a Path {
    fn to_url(self) -> CargoResult<Url> {
        Url::from_file_path(self).map_err(|()| {
            format_err!("invalid path url `{}`", self.display())
        })
    }
}
