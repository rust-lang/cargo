use crate::util::{CargoResult, ToUrl};

use url::Url;

/// A type that can be interpreted as a relative Url and converted to
/// a Url.
pub trait ToUrlWithBase {
    /// Performs the conversion
    fn to_url_with_base<U: ToUrl>(self, base: Option<U>) -> CargoResult<Url>;
}

impl<'a> ToUrlWithBase for &'a str {
    fn to_url_with_base<U: ToUrl>(self, base: Option<U>) -> CargoResult<Url> {
        let base_url = match base {
            Some(base) => Some(
                base.to_url()
                    .map_err(|s| failure::format_err!("invalid url `{}`: {}", self, s))?,
            ),
            None => None,
        };

        Url::options()
            .base_url(base_url.as_ref())
            .parse(self)
            .map_err(|s| failure::format_err!("invalid url `{}`: {}", self, s))
    }
}

#[cfg(test)]
mod tests {
    use crate::util::ToUrlWithBase;

    #[test]
    fn to_url_with_base() {
        assert_eq!(
            "rel/path"
                .to_url_with_base(Some("file:///abs/path/"))
                .unwrap()
                .to_string(),
            "file:///abs/path/rel/path"
        );
        assert_eq!(
            "rel/path"
                .to_url_with_base(Some("file:///abs/path/popped-file"))
                .unwrap()
                .to_string(),
            "file:///abs/path/rel/path"
        );
    }
}
