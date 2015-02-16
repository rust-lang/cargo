#[derive(Debug, Clone)]
pub struct Feature {
    dependencies: Vec<String>,
    features: Vec<String>,
}

impl Feature {
    pub fn new() -> Feature {
        Feature {
            dependencies: vec![],
            features: vec![],
        }
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    pub fn features(&self) -> &[String] {
        &self.features
    }

    /// Sets the list of dependencies required by this feature
    pub fn set_dependencies(mut self, dependencies: Vec<String>) -> Feature {
        self.dependencies = dependencies;
        self
    }

    /// Sets the list of features that this feature depends on
    pub fn set_features(mut self, features: Vec<String>) -> Feature {
        self.features = features;
        self
    }
}
