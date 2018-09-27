use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cargo::core::compiler::{CompileMode, Executor};
use cargo::core::manifest::TargetSourcePath;
use cargo::core::{PackageId, Target};
use cargo::ops;
use command_prelude::*;
use util::errors::CargoResult;
use util::ProcessBuilder;

/// An executor that uses a cache of pre-built artifacts to reduce unnecessary
/// compilation.
pub struct ChromeOSExecutor {
    // The triple for the machine on which cargo is currently running.
    host_triple: String,

    // The triple for the target on which the crate will run.
    target_triple: String,

    // The directory where cached rlibs are stored.
    registry_dir: Box<Path>,
}

impl ChromeOSExecutor {
    pub fn new() -> ChromeOSExecutor {
        let host_triple = env::var("CBUILD").expect("`CBUILD` environment variable not set");
        let target_triple = env::var("CHOST").expect("`CHOST` environment variable not set");

        let registry_dir = env::var("CARGO_CROS_REGISTRY_DIR")
            .map(PathBuf::from)
            .map(PathBuf::into_boxed_path)
            .expect("`CARGO_CROS_REGISTRY_DIR` environment variable not set");

        if !registry_dir.exists() {
            panic!("Cache directory {} does not exist", registry_dir.display());
        }

        ChromeOSExecutor {
            host_triple: host_triple,
            target_triple: target_triple,
            registry_dir: registry_dir,
        }
    }
}

impl Executor for ChromeOSExecutor {
    fn exec(
        &self,
        cmd: ProcessBuilder,
        pkg: &PackageId,
        target: &Target,
        mode: CompileMode,
    ) -> CargoResult<()> {
        if mode != CompileMode::Build {
            // This is not a build.
            return cmd.exec();
        }

        if !target.is_lib() {
            // We only cache library targets.
            return cmd.exec();
        }

        // Only look at the cache if the source path starts with the registry directory.
        if let TargetSourcePath::Path(ref pb) = target.src_path() {
            if !pb.starts_with(&self.registry_dir) {
                return cmd.exec();
            }
        } else {
            // This is a metabuild.  Just run the command.
            return cmd.exec();
        }

        // This is an rlib that we should already have built.  Depending on whether we are building
        // for the host or the target we will provide the appropriate pre-built.
        let triple = match cmd
            .get_args()
            .iter()
            .filter_map(|s| s.to_str())
            .position(|s| s.starts_with("--target"))
        {
            // If `--target` is not specified assume we are building for the host machine.
            None => &self.host_triple,

            // Otherwise, check to see if the target matches our `target_triple`.
            Some(idx) => {
                if cmd.get_args()[idx + 1] == OsStr::new(&self.target_triple) {
                    &self.target_triple
                } else {
                    &self.host_triple
                }
            }
        };

        let mut rlib = self.registry_dir.join(triple);
        rlib.push(format!("{}-{}", pkg.name(), pkg.version()));

        let libname = format!("lib{}.rlib", target.name());
        rlib.push(&libname);

        assert!(rlib.exists());
        assert!(rlib.is_file());

        let extra_filename = cmd
            .get_args()
            .iter()
            .filter_map(|s| s.to_str())
            .find(|s| s.starts_with("extra-filename"))
            .map(|extra| extra.split("="))
            .unwrap()
            .nth(1)
            .unwrap();

        let out_dir = OsStr::new("--out-dir");
        let out_dir_pos = cmd.get_args().iter().position(|p| p == out_dir).unwrap();
        let mut dst = PathBuf::from(cmd.get_args()[out_dir_pos + 1].to_str().unwrap());
        dst.push(format!("lib{}{}.rlib", target.name(), extra_filename));

        // Hard link the file to avoid unnecessary copying.
        fs::hard_link(rlib, dst).unwrap();

        Ok(())
    }
}

pub fn cli() -> App {
    subcommand("build")
        .alias("b")
        .about("Compile a local package and all of its dependencies")
        .arg_package_spec(
            "Package to build",
            "Build all packages in the workspace",
            "Exclude packages from the build",
        )
        .arg_jobs()
        .arg_targets_all(
            "Build only this package's library",
            "Build only the specified binary",
            "Build all binaries",
            "Build only the specified example",
            "Build all examples",
            "Build only the specified test target",
            "Build all tests",
            "Build only the specified bench target",
            "Build all benches",
            "Build all targets",
        )
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg(opt("out-dir", "Copy final artifacts to this directory").value_name("PATH"))
        .arg(opt("chromeos-executor", "Use the Chrome OS executor"))
        .arg_manifest_path()
        .arg_message_format()
        .arg_build_plan()
        .after_help(
            "\
If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are built if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    let mut compile_opts = args.compile_options(config, CompileMode::Build)?;
    compile_opts.export_dir = args.value_of_path("out-dir", config);
    if compile_opts.export_dir.is_some() && !config.cli_unstable().unstable_options {
        Err(format_err!(
            "`--out-dir` flag is unstable, pass `-Z unstable-options` to enable it"
        ))?;
    };
    if args.is_present("chromeos-executor") {
        let executor: Arc<Executor> = Arc::new(ChromeOSExecutor::new());
        ops::compile_with_exec(&ws, &compile_opts, &executor)?;
    } else {
        ops::compile(&ws, &compile_opts)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        let _c = ChromeOSExecutor::new();
    }
}
