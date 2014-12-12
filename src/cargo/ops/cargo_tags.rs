//!
//! Cargo tags generates ctags files in either emacs or vi format, using a copy
//! of exuberant that exists on the PATH.
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

    for opt in rust_cfg.iter().chain(extra_params.iter()).chain(emacs_tags.iter()) {
        ctags = ctags.arg(opt);
    }
    for path in files.iter() {
        ctags = ctags.arg(path.display().as_cow().as_slice());
    }

    log!(4, "running ctags; cmd={}", ctags);

    ctags.exec_with_output().map(|_| ()).box_error()
}
