use crate::context::ConfigValue;
use crate::util::data_structures::HashMap;

#[derive(Debug)]
pub struct ConfigValues {
    /// A collection of configuration options
    pub values: HashMap<String, ConfigValue>,
}
