use crate::util::{CargoResult, IntoUrl};

use url::Url;

/// A type that can be interpreted as a relative Url and converted to
/// a Url.
pub trait IntoUrlWithBase {
    /// Performs the conversion
    fn into_url_with_base<U: IntoUrl>(self, base: Option<U>) -> CargoResult<Url>;
}

impl<'a> IntoUrlWithBase for &'a str {
    fn into_url_with_base<U: IntoUrl>(self, base: Option<U>) -> CargoResult<Url> {
        let base_url = match base {
            Some(base) => Some(
                base.into_url()
                    .map_err(|s| anyhow::format_err!("invalid url `{}`: {}", self, s))?,
            ),
            None => None,
        };

        Url::options()
            .base_url(base_url.as_ref())
            .parse(self)
            .map_err(|s| anyhow::format_err!("invalid url `{}`: {}", self, s))
    }
}

#[cfg(test)]
mod tests {
    use crate::util::IntoUrlWithBase;

    #[test]
    fn into_url_with_base() {
        assert_eq!(
            "rel/path"
                .into_url_with_base(Some("file:///abs/path/"))
                .unwrap()
                .to_string(),
            "file:///abs/path/rel/path"
        );
        assert_eq!(
            "rel/path"
                .into_url_with_base(Some("file:///abs/path/popped-file"))
                .unwrap()
                .to_string(),
            "file:///abs/path/rel/path"
        );
    }
}
