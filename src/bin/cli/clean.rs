use super::command_prelude::*;

pub fn cli() -> App {
    subcommand("clean")
        .about("Remove artifacts that cargo has generated in the past")
        .arg(
            opt("package", "Package to clean artifacts for")
                .short("p").value_name("SPEC").multiple(true)
        )
        .arg_manifest_path()
        .arg_target_triple("Target triple to clean output for (default all)")
        .arg_release("Whether or not to clean release artifacts")
        .after_help("\
If the --package argument is given, then SPEC is a package id specification
which indicates which package's artifacts should be cleaned out. If it is not
given, then all packages' artifacts are removed. For more information on SPEC
and its format, see the `cargo help pkgid` command.
")
}
