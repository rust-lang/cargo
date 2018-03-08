use super::utils::*;

pub fn cli() -> App {
    subcommand("publish")
        .about("Upload a package to the registry")
        .arg(
            opt("index", "Registry index to upload the package to")
                .value_name("INDEX")
        )
        .arg(
            opt("host", "DEPRECATED, renamed to '--index'")
                .value_name("HOST")
                .hidden(true)
        )
        .arg(opt("token", "Token to use when uploading").value_name("TOKEN"))
        .arg(opt("no-verify", "Don't verify the contents by building them"))
        .arg(opt("allow-dirty", "Allow dirty working directories to be packaged"))
        .arg_target_triple("Build for the target triple")
        .arg_manifest_path()
        .arg_jobs()
        .arg(
            opt("dry-run", "Perform all checks without uploading")
        )
        .arg(opt("registry", "Registry to publish to").value_name("REGISTRY"))
}
