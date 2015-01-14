use cargo::util::{CliResult, CliError, human, ChainError, Config};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[derive(RustcDecodable)]
struct LocateProjectFlags {
    flag_manifest_path: Option<String>,
}

pub const USAGE: &'static str = "
Usage:
    cargo locate-project [options]

Options:
    --manifest-path PATH    Path to the manifest to build benchmarks for
    -h, --help              Print this message
";

#[derive(RustcEncodable)]
struct ProjectLocation {
    root: String
}

pub fn execute(flags: LocateProjectFlags,
               _: &Config) -> CliResult<Option<ProjectLocation>> {
    let root = try!(find_root_manifest_for_cwd(flags.flag_manifest_path));

    let string = try!(root.as_str()
                      .chain_error(|| human("Your project path contains \
                                             characters not representable in \
                                             Unicode"))
                      .map_err(|e| CliError::from_boxed(e, 1)));

    Ok(Some(ProjectLocation { root: string.to_string() }))
}
