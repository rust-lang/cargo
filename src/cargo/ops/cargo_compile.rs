/**
 * Cargo compile currently does the following steps:
 *
 * All configurations are already injected as environment variables via the main cargo command
 *
 * 1. Read the manifest
 * 2. Shell out to `cargo-resolve` with a list of dependencies and sources as stdin
 *    a. Shell out to `--do update` and `--do list` for each source
 *    b. Resolve dependencies and return a list of name/version/source
 * 3. Shell out to `--do download` for each source
 * 4. Shell out to `--do get` for each source, and build up the list of paths to pass to rustc -L
 * 5. Call `cargo-rustc` with the results of the resolver zipped together with the results of the `get`
 *    a. Topologically sort the dependencies
 *    b. Compile each dependency in order, passing in the -L's pointing at each previously compiled dependency
 */

use std::io::MemWriter;
use std::os;
use std::result;
use std::hash::sip::SipHasher;
use std::hash::Hasher;
use serialize::hex::ToHex;
use url::Url;
use util::config;
use util::config::{ConfigValue};
use core::{Package,PackageSet,Source,SourceSet};
use core::resolver::resolve;
use core::source::{GitKind,SourceId};
use core::registry::PackageRegistry;
use sources::{PathSource,GitSource};
use sources::git::GitRemote;
use ops;
use util::{CargoResult, Wrap, Require, simple_human, other_error};

pub fn compile(manifest_path: &Path) -> CargoResult<()> {
    log!(4, "compile; manifest-path={}", manifest_path.display());

    // TODO: Move this into PathSource
    let package = try!(PathSource::read_package(manifest_path));
    debug!("loaded package; package={}", package);

    let overrides = try!(sources_from_config());
    let sources = try!(sources_for(&package));

    let registry = PackageRegistry::new(sources, overrides);

    //try!(sources.update().wrap("unable to update sources"));
    //let summaries = try!(sources.list().wrap("unable to list packages from source"));

    //let registry = PackageRegistry::new(&summaries, &overrides);

    //let resolved = try!(resolve(package.get_dependencies(), &summaries).wrap("unable to resolve dependencies"));

    //try!(sources.download(resolved.as_slice()).wrap("unable to download packages"));

    //let packages = try!(sources.get(resolved.as_slice()).wrap("unable to get packages from source"));

    //log!(5, "fetch packages from source; packages={}; ids={}", packages, resolved);

    //let package_set = PackageSet::new(packages.as_slice());

    //try!(ops::compile_packages(&package, &package_set));

    Ok(())
}

fn sources_for(package: &Package) -> CargoResult<Vec<Box<Source>>> {
    let mut sources = vec!(box PathSource::new(vec!(package.get_manifest_path().dir_path())) as Box<Source>);

    let git_sources: Vec<Box<Source>> = try!(result::collect(package.get_sources().iter().map(|source_id: &SourceId| {
        match source_id.kind {
            GitKind(ref reference) => {
                let remote = GitRemote::new(source_id.url.clone(), false);
                let home = try!(os::homedir().require(simple_human("Cargo couldn't find a home directory")));
                let git = home.join(".cargo").join("git");
                let ident = url_to_path_ident(&source_id.url);

                // .cargo/git/db
                // .cargo/git/checkouts
                let db_path = git.join("db").join(ident.as_slice());
                let checkout_path = git.join("checkouts").join(ident.as_slice()).join(reference.as_slice());
                Ok(box GitSource::new(remote, reference.clone(), db_path, checkout_path) as Box<Source>)
            },
            ref PathKind => fail!("Cannot occur")
        }
    })));

    sources.push_all_move(git_sources);

    Ok(sources)
}

fn sources_from_config() -> CargoResult<SourceSet> {
    let configs = try!(config::all_configs(os::getcwd()));

    debug!("loaded config; configs={}", configs);

    let config_paths = configs.find_equiv(&"paths").map(|v| v.clone()).unwrap_or_else(|| ConfigValue::new());

    let mut paths: Vec<Path> = match config_paths.get_value() {
        &config::String(_) => return Err(other_error("The path was configured as a String instead of a List")),
        &config::List(ref list) => list.iter().map(|path| Path::new(path.as_slice())).collect()
    };

    Ok(SourceSet::new(vec!(box PathSource::new(paths) as Box<Source>)))
}

fn url_to_path_ident(url: &Url) -> String {
    let hasher = SipHasher::new_with_keys(0,0);

    let mut ident = url.path.as_slice().split('/').last().unwrap();

    ident = if ident == "" {
        "_empty"
    } else {
        ident
    };

    format!("{}-{}", ident, to_hex(hasher.hash(&url.to_str())))
}

fn to_hex(num: u64) -> String {
    let mut writer = MemWriter::with_capacity(8);
    writer.write_le_u64(num).unwrap(); // this should never fail
    writer.get_ref().to_hex()
}

#[cfg(test)]
mod test {
    use url;
    use url::Url;
    use super::url_to_path_ident;

    #[test]
    pub fn test_url_to_path_ident_with_path() {
        let ident = url_to_path_ident(&url("https://github.com/carlhuda/cargo"));
        assert_eq!(ident.as_slice(), "cargo-0eed735c8ffd7c88");
    }

    #[test]
    pub fn test_url_to_path_ident_without_path() {
        let ident = url_to_path_ident(&url("https://github.com"));
        assert_eq!(ident.as_slice(), "_empty-fc065c9b6b16fc00");
    }


    fn url(s: &str) -> Url {
        url::from_str(s).unwrap()
    }
}
