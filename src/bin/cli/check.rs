use super::utils::*;

pub fn cli() -> App {
    subcommand("check")
        .about("Check a local package and all of its dependencies for errors")
        .arg_package(
            "Package(s) to check",
            "Check all packages in the workspace",
            "Exclude packages from the check",
        )
        .arg_jobs()
        .arg_targets_all(
            "Check only this package's library",
            "Check only the specified binary",
            "Check all binaries",
            "Check only the specified example",
            "Check all examples",
            "Check only the specified test target",
            "Check all tests",
            "Check only the specified bench target",
            "Check all benches",
            "Check all targets (lib and bin targets by default)",
        )
        .arg_release("Check artifacts in release mode, with optimizations")
        .arg(
            opt("profile", "Profile to build the selected target for")
                .value_name("PROFILE")
        )
        .arg_features()
        .arg_target_triple("Check for the target triple")
        .arg_manifest_path()
        .arg_message_format()
        .arg_locked()
        .after_help("\
If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are checked if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.

The `--profile test` flag can be used to check unit tests with the
`#[cfg(test)]` attribute.
")
}
