use crate::core::features::AllowFeatures;
use crate::core::features::UnstableFeatureContext;
use crate::Config;

impl UnstableFeatureContext for Config {
    fn nightly_features_allowed(&self) -> bool {
        self.nightly_features_allowed
    }

    fn allow_features(&self) -> Option<&AllowFeatures> {
        self.unstable_flags.allow_features.as_ref()
    }
}
