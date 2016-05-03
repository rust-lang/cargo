use url::Url;
use std::path::Path;

pub trait ToUrl {
    fn to_url(self) -> Result<Url, String>;
}

impl ToUrl for Url {
    fn to_url(self) -> Result<Url, String> {
        Ok(self)
    }
}

impl<'a> ToUrl for &'a Url {
    fn to_url(self) -> Result<Url, String> {
        Ok(self.clone())
    }
}

impl<'a> ToUrl for &'a str {
    fn to_url(self) -> Result<Url, String> {
        Url::parse(self).map_err(|s| {
            format!("invalid url `{}`: {}", self, s)
        })
    }
}

impl<'a> ToUrl for &'a Path {
    fn to_url(self) -> Result<Url, String> {
        Url::from_file_path(self).map_err(|()| {
            format!("invalid path url `{}`", self.display())
        })
    }
}
