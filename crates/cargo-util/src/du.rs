//! A simple disk usage estimator.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use ignore::overrides::OverrideBuilder;
use ignore::{WalkBuilder, WalkState};
use portable_atomic::AtomicU64;

/// Determines the disk usage of all files in the given directory.
///
/// The given patterns are gitignore style patterns relative to the given
/// path. If there are patterns, it will only count things matching that
/// pattern. `!` can be used to exclude things. See [`OverrideBuilder::add`]
/// for more info.
///
/// This is a primitive implementation that doesn't handle hard links, and
/// isn't particularly fast (for example, not using `getattrlistbulk` on
/// macOS). It also only uses actual byte sizes instead of block counts (and
/// thus vastly undercounts directories with lots of small files). It would be
/// nice to improve this or replace it with something better.
pub fn du(path: &Path, patterns: &[&str]) -> Result<u64> {
    Ok(du_each(path, &[path], patterns)?[0])
}

/// Determines the disk usage of each of the given directories, in order.
///
/// The patterns are as for [`du`], relative to `root`, which every path must
/// be within. The paths must not be nested within one another.
///
/// A walk builds a thread pool of its own, which for a small directory costs
/// far more than the walk itself. Prefer this over calling [`du`] in a loop:
/// it walks every path with a single pool.
pub fn du_each(root: &Path, paths: &[&Path], patterns: &[&str]) -> Result<Vec<u64>> {
    du_each_inner(root, paths, patterns)
        .with_context(|| format!("failed to walk `{}`", root.display()))
}

fn du_each_inner(root: &Path, paths: &[&Path], patterns: &[&str]) -> Result<Vec<u64>> {
    let Some((first, rest)) = paths.split_first() else {
        return Ok(Vec::new());
    };

    let mut builder = OverrideBuilder::new(root);
    for pattern in patterns {
        builder.add(pattern)?;
    }
    let overrides = builder.build()?;

    let mut builder = WalkBuilder::new(first);

    for path in rest {
        builder.add(path);
    }

    builder
        .overrides(overrides)
        .hidden(false)
        .parents(false)
        .ignore(false)
        .git_global(false)
        .git_ignore(false)
        .git_exclude(false);
    let walker = builder.build_parallel();

    // BTreeMap, not HashMap, to stay off a faster-hasher dependency; this
    // per-file lookup is dwarfed by the `stat` on each entry regardless.
    let path_to_index: BTreeMap<&Path, usize> = paths.iter().copied().zip(0..).collect();
    let total: Vec<_> = paths.iter().map(|_| AtomicU64::new(0)).collect();

    // A slot used to indicate there was an error while walking.
    //
    // It is possible that more than one error happens (such as in different
    // threads). The error returned is arbitrary in that case.
    let err = Mutex::new(None);
    walker.run(|| {
        Box::new(|entry| {
            match entry {
                Ok(entry) => match entry.metadata() {
                    Ok(meta) => {
                        if meta.is_file() {
                            // Attribute each file to the directory it belongs under.
                            // An entry's depth is counted from the path it was
                            // reached through, which is that many levels up.
                            let path = entry.path().ancestors().nth(entry.depth());

                            if let Some(index) = path.and_then(|path| path_to_index.get(path)) {
                                total[*index].fetch_add(meta.len(), Ordering::Relaxed);
                            }
                        }
                    }
                    Err(e) => {
                        *err.lock().unwrap() = Some(e.into());
                        return WalkState::Quit;
                    }
                },
                Err(e) => {
                    *err.lock().unwrap() = Some(e.into());
                    return WalkState::Quit;
                }
            }
            WalkState::Continue
        })
    });

    if let Some(e) = err.lock().unwrap().take() {
        return Err(e);
    }

    Ok(total
        .iter()
        .map(|total| total.load(Ordering::Relaxed))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates `<root>/<name>/{Cargo.toml, src/lib.rs, .git/objects}`,
    /// each file holding a distinct number of bytes.
    fn package(root: &Path, name: &str, size: u64) {
        let dir = root.join(name);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join("Cargo.toml"), vec![b'x'; size as usize]).unwrap();
        fs::write(dir.join("src/lib.rs"), vec![b'x'; size as usize * 2]).unwrap();
        fs::write(dir.join(".git/objects"), vec![b'x'; size as usize * 100]).unwrap();
    }

    #[test]
    fn du_each_totals_each_path_separately() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        package(root, "a", 1);
        package(root, "b", 10);
        package(root, "c", 100);

        let paths = [&root.join("a"), &root.join("b"), &root.join("c")];
        let paths: Vec<_> = paths.iter().map(|path| path.as_path()).collect();
        assert_eq!(du_each(root, &paths, &[]).unwrap(), [103, 1030, 10300]);

        // Walking each path on its own must agree.
        let separately: Vec<_> = paths.iter().map(|path| du(path, &[]).unwrap()).collect();
        assert_eq!(du_each(root, &paths, &[]).unwrap(), separately);
    }

    /// Patterns are anchored at the root shared by every path, rather than at
    /// each path, so make sure they still apply within each of them.
    #[test]
    fn du_each_applies_patterns_within_each_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        package(root, "a", 1);
        package(root, "b", 10);

        let paths = [&root.join("a"), &root.join("b")];
        let paths: Vec<_> = paths.iter().map(|path| path.as_path()).collect();
        assert_eq!(du_each(root, &paths, &["!.git"]).unwrap(), [3, 30]);

        let separately: Vec<_> = paths
            .iter()
            .map(|path| du(path, &["!.git"]).unwrap())
            .collect();
        assert_eq!(du_each(root, &paths, &["!.git"]).unwrap(), separately);
    }

    #[test]
    fn du_each_of_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(du_each(tmp.path(), &[], &[]).unwrap(), []);
    }
}
