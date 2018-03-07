use super::utils::*;

pub fn cli() -> App {
    subcommand("locate-project")
        .about("Checkout a copy of a Git repository")
        .arg_manifest_path()
}
