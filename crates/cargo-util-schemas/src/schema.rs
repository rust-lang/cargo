use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

use toml::Value as TomlValue;

#[derive(Serialize, Deserialize)]
pub struct TomlValueWrapper(pub TomlValue);

impl JsonSchema for TomlValueWrapper {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TomlValue".into()
    }

    fn json_schema(gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        // HACK: this is both more and less permissive than `TomlValue` but its close
        gen.subschema_for::<serde_json::Value>().into()
    }
}
