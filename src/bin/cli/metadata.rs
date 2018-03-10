use command_prelude::*;

pub fn cli() -> App {
    subcommand("metadata")
        .about("Output the resolved dependencies of a project, \
                the concrete used versions including overrides, \
                in machine-readable format")
        .arg_features()
        .arg(
            opt("no-deps", "Output information only about the root package \
                            and don't fetch dependencies")
        )
        .arg_manifest_path()
        .arg(
            opt("format-version", "Format version")
                .value_name("VERSION").possible_value("1")
        )
}
