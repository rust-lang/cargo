use super::utils::*;

pub fn cli() -> App {
    subcommand("read-manifest")
        .about("Deprecated, use `cargo metadata --no-deps` instead.
Print a JSON representation of a Cargo.toml manifest.")
        .arg_manifest_path()
}
