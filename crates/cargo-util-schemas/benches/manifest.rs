use cargo_util_schemas::manifest::TomlManifest;

fn main() {
    divan::main();
}

#[divan::bench]
fn parse_tokio_manifest() -> TomlManifest {
    let manifest = include_str!("fixtures/tokio-1.53.2.toml");
    toml::from_str(divan::black_box(manifest)).unwrap()
}
