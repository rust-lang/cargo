use super::utils::*;

pub fn cli() -> App {
    subcommand("verify-project")
        .about("Check correctness of crate manifest")
        .arg_manifest_path()
}
