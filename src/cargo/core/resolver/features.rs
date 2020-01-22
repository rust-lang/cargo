use crate::core::resolver::types::FeaturesSet;
use crate::core::InternedString;
use std::collections::BTreeSet;
use std::rc::Rc;

/// Features flags requested for a package.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RequestedFeatures {
    pub features: FeaturesSet,
    pub all_features: bool,
    pub uses_default_features: bool,
}

impl RequestedFeatures {
    /// Creates a new RequestedFeatures from the given command-line flags.
    pub fn from_command_line(
        features: &[String],
        all_features: bool,
        uses_default_features: bool,
    ) -> RequestedFeatures {
        RequestedFeatures {
            features: Rc::new(RequestedFeatures::split_features(features)),
            all_features,
            uses_default_features,
        }
    }

    /// Creates a new RequestedFeatures with the given `all_features` setting.
    pub fn new_all(all_features: bool) -> RequestedFeatures {
        RequestedFeatures {
            features: Rc::new(BTreeSet::new()),
            all_features,
            uses_default_features: true,
        }
    }

    fn split_features(features: &[String]) -> BTreeSet<InternedString> {
        features
            .iter()
            .flat_map(|s| s.split_whitespace())
            .flat_map(|s| s.split(','))
            .filter(|s| !s.is_empty())
            .map(InternedString::new)
            .collect::<BTreeSet<InternedString>>()
    }
}
