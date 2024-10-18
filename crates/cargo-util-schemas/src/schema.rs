use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::string::String;

use toml::Value as TomlValue;

#[derive(Serialize, Deserialize)]
pub struct TomlValueWrapper(pub TomlValue);

impl JsonSchema for TomlValueWrapper {
    fn schema_name() -> String {
        "TomlValue".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::*;

        SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            object: Some(Box::new(ObjectValidation {
                properties: [
                    (
                        "string".to_string(),
                        gen.subschema_for::<std::string::String>(),
                    ),
                    ("integer".to_string(), gen.subschema_for::<i64>()),
                    ("float".to_string(), gen.subschema_for::<f64>()),
                    ("boolean".to_string(), gen.subschema_for::<bool>()),
                    (
                        "datetime".to_string(),
                        gen.subschema_for::<std::string::String>(),
                    ), // Assuming datetime is represented as a string
                    (
                        "array".to_string(),
                        gen.subschema_for::<Vec<TomlValueWrapper>>(),
                    ),
                    (
                        "table".to_string(),
                        gen.subschema_for::<HashMap<std::string::String, TomlValueWrapper>>(),
                    ),
                ]
                .iter()
                .cloned()
                .collect(),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}
