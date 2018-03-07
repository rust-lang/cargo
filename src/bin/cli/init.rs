use super::utils::*;

pub fn cli() -> App {
    subcommand("init")
        .about("Create a new cargo package in an existing directory")
        .arg(Arg::with_name("path"))
        .arg(
            opt("vcs", "\
Initialize a new repository for the given version \
control system (git, hg, pijul, or fossil) or do not \
initialize any version control at all (none), overriding \
a global configuration."
            ).value_name("VCS").possible_values(
                &["git", "hg", "pijul", "fossil", "none"]
            )
        )
        .arg(
            opt("bin", "Use a binary (application) template [default]")
        )
        .arg(
            opt("lib", "Use a library template")
        )
        .arg(
            opt("name", "Set the resulting package name")
                .value_name("NAME")
        )
        .arg_locked()
}
