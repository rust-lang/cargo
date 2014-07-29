use ops;
use util::CargoResult;

pub struct DocOptions<'a> {
    pub all: bool,
    pub compile_opts: ops::CompileOptions<'a>,
}

pub fn doc(manifest_path: &Path,
           options: &mut DocOptions) -> CargoResult<()> {
    try!(ops::compile(manifest_path, &mut options.compile_opts));
    Ok(())
}
