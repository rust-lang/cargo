use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

use std::string::String;

use toml::Value as TomlValue;

#[derive(Serialize, Deserialize)]
pub struct TomlValueWrapper(pub TomlValue);

impl JsonSchema for TomlValueWrapper {
    fn schema_name() -> String {
        "TomlValue".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        // HACK: this is both more and less permissive than `TomlValue` but its close
        gen.subschema_for::<serde_json::Value>().into()
    }
}
