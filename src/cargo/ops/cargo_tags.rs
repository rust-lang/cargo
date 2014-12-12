//!
//! Cargo compile currently does the following steps:
//!
//! All configurations are already injected as environment variables via the
//! main cargo command
//!
//! 1. Read the manifest
//! 2. Shell out to `cargo-resolve` with a list of dependencies and sources as
//!    stdin
//!
//!    a. Shell out to `--do update` and `--do list` for each source
//!    b. Resolve dependencies and return a list of name/version/source
//!
//! 3. Shell out to `--do download` for each source
//! 4. Shell out to `--do get` for each source, and build up the list of paths
//!    to pass to rustc -L
//! 5. Call `cargo-rustc` with the results of the resolver zipped together with
//!    the results of the `get`
//!
//!    a. Topologically sort the dependencies
//!    b. Compile each dependency in order, passing in the -L's pointing at each
//!       previously compiled dependency
//!

use sources::PathSource;
use util::{mod, CargoResult, BoxError};

/// Contains informations about how a package should be compiled.
pub struct TagsOptions {
    pub emacs_tags: bool,
}

fn to_display(files: &Vec<Path>) -> Vec<String> {
    (*files).clone().into_iter().map(|f| f.display().to_string()).collect()
}

pub fn generate_tags(manifest_path: &Path, options: &mut TagsOptions) -> CargoResult<()> {
    log!(4, "tags; manifest-path={}", manifest_path.display());

    let source = try!(PathSource::for_path(&manifest_path.dir_path()));
    let packages = try!(source.read_packages());

    let mut files = vec!();

    for p in packages.iter() {
        files.extend(try!(source.list_files(p)).into_iter());
    }

    log!(4, "generating tags; paths={}", to_display(&files));

    let mut ctags = try!(util::process("ctags"));

    let rust_cfg: [&'static str, ..11] = include!("ctags.rust");
    let extra_params = [ "--languages=Rust", "--recurse" ];
    let emacs_tags = if options.emacs_tags { Some("-e") } else { None };

    for opt in rust_cfg.iter().chain(extra_params.iter()).chain(vi_tags.iter()) {
        ctags = ctags.arg(opt);
    }
    for path in files.iter() {
        ctags = ctags.arg(path.display().as_cow().as_slice());
    }

    log!(4, "running ctags; cmd={}", ctags);

    ctags.exec_with_output().map(|_| ()).box_error()
}
