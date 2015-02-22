use ops;
use util::CargoResult;
use sources::PathSource;
use std::path::Path;

pub fn install(manifest_path: &Path,
               opts: &ops::CompileOptions) -> CargoResult<()> {
    let config = opts.config;
    let src = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                            config));
    let _root = try!(src.root_package());

    println!("Compiling");
    try!(ops::compile(manifest_path, opts));

    Ok(())
}
