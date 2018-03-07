use super::utils::*;

pub fn cli() -> App {
    subcommand("package")
        .about("Assemble the local package into a distributable tarball")
        .arg(opt("list", "Print files included in a package without making one").short("l"))
        .arg(opt("no-verify", "Don't verify the contents by building them"))
        .arg(opt("no-metadata", "Ignore warnings about a lack of human-usable metadata"))
        .arg(opt("allow-dirty", "Allow dirty working directories to be packaged"))
        .arg_target_triple("Build for the target triple")
        .arg_manifest_path()
        .arg_jobs()
}
