use std::path::{Path, PathBuf};

use url::Url;

use crate::util::CargoResult;

/// A type that can be converted to a Url
pub trait IntoUrl {
    /// Performs the conversion
    fn into_url(self) -> CargoResult<Url>;
}

impl<'a> IntoUrl for &'a str {
    fn into_url(self) -> CargoResult<Url> {
        Url::parse(self).map_err(|s| {
            if self.starts_with("git@") {
                anyhow::format_err!(
                    "invalid url `{}`: {}; try using `{}` instead",
                    self,
                    s,
                    format_args!("ssh://{}", self.replacen(':', "/", 1))
                )
            } else {
                anyhow::format_err!("invalid url `{}`: {}", self, s)
            }
        })
    }
}

impl<'a> IntoUrl for &'a Path {
    fn into_url(self) -> CargoResult<Url> {
        Url::from_file_path(self)
            .map_err(|()| anyhow::format_err!("invalid path url `{}`", self.display()))
    }
}

impl<'a> IntoUrl for &'a PathBuf {
    fn into_url(self) -> CargoResult<Url> {
        self.as_path().into_url()
    }
}
