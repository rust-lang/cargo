use crate::context::ConfigValue;
use crate::util::data_structures::HashMap;

#[derive(Default, Debug)]
pub struct ConfigValues {
    /// A collection of configuration options
    values: HashMap<String, ConfigValue>,
}

impl ConfigValues {
    pub fn with_values(values: HashMap<String, ConfigValue>) -> Self {
        Self { values }
    }

    pub fn values(&self) -> &HashMap<String, ConfigValue> {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut HashMap<String, ConfigValue> {
        &mut self.values
    }
}
