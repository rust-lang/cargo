use cargo::core::registry::PackageRegistry;
use cargo::core::QueryKind;
use cargo::core::Registry;
use cargo::core::SourceId;
use cargo::util::command_prelude::*;

pub fn cli() -> clap::Command {
    clap::Command::new("unpublished")
}

pub fn exec(args: &clap::ArgMatches, config: &mut cargo::util::Config) -> cargo::CliResult {
    let ws = args.workspace(config)?;
    let mut results = Vec::new();
    {
        let mut registry = PackageRegistry::new(config)?;
        let _lock = config.acquire_package_cache_lock()?;
        registry.lock_patches();
        let source_id = SourceId::crates_io(config)?;

        for member in ws.members() {
            let name = member.name();
            let current = member.version();
            if member.publish() == &Some(vec![]) {
                log::trace!("skipping {name}, `publish = false`");
                continue;
            }

            let version_req = format!("<={current}");
            let query = cargo::core::dependency::Dependency::parse(
                name,
                Some(&version_req),
                source_id.clone(),
            )?;
            let possibilities = loop {
                // Exact to avoid returning all for path/git
                match registry.query_vec(&query, QueryKind::Exact) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            };
            if let Some(last) = possibilities.iter().map(|s| s.version()).max() {
                if last != current {
                    results.push((
                        name.to_string(),
                        Some(last.to_string()),
                        current.to_string(),
                    ));
                } else {
                    log::trace!("{name} {current} is published");
                }
            } else {
                results.push((name.to_string(), None, current.to_string()));
            }
        }
    }

    if !results.is_empty() {
        results.insert(
            0,
            (
                "name".to_owned(),
                Some("published".to_owned()),
                "current".to_owned(),
            ),
        );
        results.insert(
            1,
            (
                "====".to_owned(),
                Some("=========".to_owned()),
                "=======".to_owned(),
            ),
        );
    }
    for (name, last, current) in results {
        if let Some(last) = last {
            println!("{name} {last} {current}");
        } else {
            println!("{name} - {current}");
        }
    }

    Ok(())
}
