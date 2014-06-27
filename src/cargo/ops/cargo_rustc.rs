use std::os::args;
use std::io;
use std::io::{File, IoError};
use std::str;
use std::hash::sip::SipHasher;
use std::hash::Hasher;

use core::{Package, PackageSet, Target};
use util;
use util::{CargoResult, ChainError, ProcessBuilder, internal, human, CargoError};
use util::{Config};

type Args = Vec<String>;

struct Context<'a, 'b> {
    dest: &'a Path,
    deps_dir: &'a Path,
    primary: bool,
    rustc_version: &'a str,
    compiled_anything: bool,
    config: &'b mut Config<'b>
}

pub fn compile_targets<'a>(targets: &[&Target], pkg: &Package, deps: &PackageSet,
                           config: &'a mut Config<'a>) -> CargoResult<()> {

    debug!("compile_targets; targets={}; pkg={}; deps={}", targets, pkg, deps);

    let target_dir = pkg.get_absolute_target_dir();
    let deps_target_dir = target_dir.join("deps");
    let tests_target_dir = target_dir.join("tests");

    let output = try!(util::process("rustc").arg("-v").exec_with_output());
    let rustc_version = str::from_utf8(output.output.as_slice()).unwrap();

    // First ensure that the destination directory exists
    debug!("creating target dir; path={}", target_dir.display());

    try!(mk_target(&target_dir).chain_error(||
        internal(format!("Couldn't create the target directory for {} at {}",
                 pkg.get_name(), target_dir.display()))));

    try!(mk_target(&deps_target_dir).chain_error(||
        internal(format!("Couldn't create the directory for dependencies for {} at {}",
                 pkg.get_name(), deps_target_dir.display()))));

    try!(mk_target(&tests_target_dir).chain_error(||
        internal(format!("Couldn't create the directory for tests for {} at {}",
                 pkg.get_name(), tests_target_dir.display()))));

    let mut cx = Context {
        dest: &deps_target_dir,
        deps_dir: &deps_target_dir,
        primary: false,
        rustc_version: rustc_version.as_slice(),
        compiled_anything: false,
        config: config
    };

    // Traverse the dependencies in topological order
    for dep in try!(topsort(deps)).iter() {
        let targets = dep.get_targets().iter().filter(|target| {
            // Only compile lib targets for dependencies
            target.is_lib() && target.get_profile().is_compile()
        }).collect::<Vec<&Target>>();

        try!(compile(targets.as_slice(), dep, &mut cx));
    }

    cx.primary = true;
    cx.dest = &target_dir;

    try!(compile(targets, pkg, &mut cx));

    Ok(())
}

fn compile(targets: &[&Target], pkg: &Package, cx: &mut Context) -> CargoResult<()> {
    debug!("compile_pkg; pkg={}; targets={}", pkg, pkg.get_targets());

    if targets.is_empty() {
        return Ok(());
    }

    // First check to see if this package is fresh.
    //
    // Note that we're compiling things in topological order, so if nothing has
    // been built up to this point and we're fresh, then we can safely skip
    // recompilation. If anything has previously been rebuilt, it may have been
    // a dependency of ours, so just go ahead and rebuild ourselves.
    //
    // This is not quite accurate, we should only trigger forceful
    // recompilations for downstream dependencies of ourselves, not everyone
    // compiled afterwards.a
    //
    // TODO: Figure out how this works with targets
    let fingerprint_loc = cx.dest.join(format!(".{}.fingerprint",
                                               pkg.get_name()));
    let (is_fresh, fingerprint) = try!(is_fresh(pkg, &fingerprint_loc, cx, targets));
    if !cx.compiled_anything && is_fresh {
        try!(cx.config.shell().status("Fresh", pkg));
        return Ok(())
    }

    // Alright, so this package is not fresh and we need to compile it. Start
    // off by printing a nice helpful message and then run the custom build
    // command if one is present.
    try!(cx.config.shell().status("Compiling", pkg));

    // TODO: Should this be on the target or the package?
    match pkg.get_manifest().get_build() {
        Some(cmd) => try!(compile_custom(pkg, cmd, cx)),
        None => {}
    }

    // After the custom command has run, execute rustc for all targets of our
    // package.
    for &target in targets.iter() {
        try!(rustc(&pkg.get_root(), target, cx));
    }

    // Now that everything has successfully compiled, write our new fingerprint
    // to the relevant location to prevent recompilations in the future.
    try!(File::create(&fingerprint_loc).write_str(fingerprint.as_slice()));
    cx.compiled_anything = true;

    Ok(())
}

fn is_fresh(dep: &Package, loc: &Path,
            cx: &mut Context, targets: &[&Target]) -> CargoResult<(bool, String)>
{
    let new_pkg_fingerprint = format!("{}{}", cx.rustc_version,
                                  try!(dep.get_fingerprint(cx.config)));

    let new_fingerprint = fingerprint(new_pkg_fingerprint, hash_targets(targets));

    let mut file = match File::open(loc) {
        Ok(file) => file,
        Err(..) => return Ok((false, new_fingerprint)),
    };

    let old_fingerprint = try!(file.read_to_str());

    log!(5, "old fingerprint: {}", old_fingerprint);
    log!(5, "new fingerprint: {}", new_fingerprint);

    Ok((old_fingerprint == new_fingerprint, new_fingerprint))
}

fn hash_targets(targets: &[&Target]) -> u64 {
    let hasher = SipHasher::new_with_keys(0,0);
    let targets = targets.iter().map(|t| (*t).clone()).collect::<Vec<Target>>();
    hasher.hash(&targets)
}

fn fingerprint(package: String, profiles: u64) -> String {
    let hasher = SipHasher::new_with_keys(0,0);
    util::to_hex(hasher.hash(&(package, profiles)))
}

fn mk_target(target: &Path) -> Result<(), IoError> {
    io::fs::mkdir_recursive(target, io::UserRWX)
}

fn compile_custom(pkg: &Package, cmd: &str, cx: &Context) -> CargoResult<()> {
    // FIXME: this needs to be smarter about splitting
    let mut cmd = cmd.split(' ');
    let mut p = util::process(cmd.next().unwrap())
                     .cwd(pkg.get_root())
                     .env("OUT_DIR", Some(cx.dest.as_str().expect("non-UTF8 dest path")))
                     .env("DEPS_DIR", Some(cx.dest.join(cx.deps_dir)
                                             .as_str().expect("non-UTF8 deps path")));
    for arg in cmd {
        p = p.arg(arg);
    }
    p.exec_with_output().map(|_| ()).map_err(|e| e.mark_human())
}

fn rustc(root: &Path, target: &Target, cx: &Context) -> CargoResult<()> {

    let crate_types = target.rustc_crate_types();

    log!(5, "root={}; target={}; crate_types={}; dest={}; deps={}; verbose={}",
         root.display(), target, crate_types, cx.dest.display(),
         cx.deps_dir.display(), cx.primary);

    let rustc = prepare_rustc(root, target, crate_types, cx);

    try!(if cx.primary {
        rustc.exec().map_err(|err| human(err.to_str()))
    } else {
        rustc.exec_with_output().and(Ok(())).map_err(|err| human(err.to_str()))
    });

    Ok(())
}

fn prepare_rustc(root: &Path, target: &Target, crate_types: Vec<&str>,
                 cx: &Context) -> ProcessBuilder {
    let mut args = Vec::new();

    build_base_args(&mut args, target, crate_types, cx);
    build_deps_args(&mut args, cx);


    util::process("rustc")
        .cwd(root.clone())
        .args(args.as_slice())
        .env("RUST_LOG", None) // rustc is way too noisy
}

fn build_base_args(into: &mut Args, target: &Target, crate_types: Vec<&str>,
                   cx: &Context) {
    // TODO: Handle errors in converting paths into args
    into.push(target.get_path().display().to_str());
    for crate_type in crate_types.iter() {
        into.push("--crate-type".to_str());
        into.push(crate_type.to_str());
    }

    let mut out = cx.dest.clone();

    if target.get_profile().is_test() {
        into.push("--test".to_str());
        out = out.join("tests");
    }

    into.push("--out-dir".to_str());
    into.push(out.display().to_str());
}

fn build_deps_args(dst: &mut Args, cx: &Context) {
    dst.push("-L".to_str());
    dst.push(cx.dest.display().to_str());
    dst.push("-L".to_str());
    dst.push(cx.deps_dir.display().to_str());
}

fn topsort(deps: &PackageSet) -> CargoResult<PackageSet> {
    match deps.sort() {
        Some(deps) => Ok(deps),
        None => return Err(internal("circular dependency detected"))
    }
}
