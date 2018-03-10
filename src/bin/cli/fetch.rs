use command_prelude::*;

pub fn cli() -> App {
    subcommand("fetch")
        .about("Fetch dependencies of a package from the network")
        .arg_manifest_path()
        .after_help("\
If a lockfile is available, this command will ensure that all of the git
dependencies and/or registries dependencies are downloaded and locally
available. The network is never touched after a `cargo fetch` unless
the lockfile changes.

If the lockfile is not available, then this is the equivalent of
`cargo generate-lockfile`. A lockfile is generated and dependencies are also
all updated.
")
}
