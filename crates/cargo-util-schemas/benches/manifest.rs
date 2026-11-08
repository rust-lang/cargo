use cargo_util_schemas::manifest::TomlManifest;
use serde::Deserialize;
use toml::de::{DeTable, Deserializer};

const TOKIO_MANIFEST: &str = include_str!("fixtures/tokio-1.53.2.toml");

#[divan::bench]
fn toml_from_str_tokio() -> TomlManifest {
    let manifest = TOKIO_MANIFEST;
    toml::from_str(divan::black_box(manifest)).unwrap()
}

#[divan::bench]
fn de_table_parse_deserialize_tokio() -> TomlManifest {
    let document = DeTable::parse(divan::black_box(TOKIO_MANIFEST)).unwrap();
    TomlManifest::deserialize(Deserializer::from(document)).unwrap()
}

#[divan::bench]
fn de_table_parse_make_owned_deserialize_tokio() -> TomlManifest {
    let mut document = DeTable::parse(divan::black_box(TOKIO_MANIFEST)).unwrap();
    document.get_mut().make_owned();
    TomlManifest::deserialize(Deserializer::from(document)).unwrap()
}

fn main() {
    divan::main();
}
