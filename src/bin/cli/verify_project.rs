use super::command_prelude::*;

pub fn cli() -> App {
    subcommand("verify-project")
        .about("Check correctness of crate manifest")
        .arg_manifest_path()
}
