//! Interacts with the registry [search API][1].
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#search

use std::cmp;
use std::iter::repeat;

use anyhow::Context as _;
use url::Url;

use crate::util::style;
use crate::util::truncate_with_ellipsis;
use crate::CargoResult;
use crate::Config;

use super::RegistryOrIndex;

pub fn search(
    query: &str,
    config: &Config,
    reg_or_index: Option<RegistryOrIndex>,
    limit: u32,
) -> CargoResult<()> {
    let (mut registry, source_ids) =
        super::registry(config, None, reg_or_index.as_ref(), false, None)?;
    let (crates, total_crates) = registry.search(query, limit).with_context(|| {
        format!(
            "failed to retrieve search results from the registry at {}",
            registry.host()
        )
    })?;

    let names = crates
        .iter()
        .map(|krate| format!("{} = \"{}\"", krate.name, krate.max_version))
        .collect::<Vec<String>>();

    let description_margin = names.iter().map(|s| s.len() + 4).max().unwrap_or_default();

    let description_length = cmp::max(80, 128 - description_margin);

    let descriptions = crates.iter().map(|krate| {
        krate
            .description
            .as_ref()
            .map(|desc| truncate_with_ellipsis(&desc.replace("\n", " "), description_length))
    });

    for (name, description) in names.into_iter().zip(descriptions) {
        let line = match description {
            Some(desc) => {
                let space = repeat(' ')
                    .take(description_margin - name.len())
                    .collect::<String>();
                name + &space + "# " + &desc
            }
            None => name,
        };
        let mut fragments = line.split(query).peekable();
        while let Some(fragment) = fragments.next() {
            let _ = config.shell().write_stdout(fragment, &style::NOP);
            if fragments.peek().is_some() {
                let _ = config.shell().write_stdout(query, &style::GOOD);
            }
        }
        let _ = config.shell().write_stdout("\n", &style::NOP);
    }

    let search_max_limit = 100;
    if total_crates > limit && limit < search_max_limit {
        let _ = config.shell().write_stdout(
            format_args!(
                "... and {} crates more (use --limit N to see more)\n",
                total_crates - limit
            ),
            &style::NOP,
        );
    } else if total_crates > limit && limit >= search_max_limit {
        let extra = if source_ids.original.is_crates_io() {
            let url = Url::parse_with_params("https://crates.io/search", &[("q", query)])?;
            format!(" (go to {url} to see more)")
        } else {
            String::new()
        };
        let _ = config.shell().write_stdout(
            format_args!("... and {} crates more{}\n", total_crates - limit, extra),
            &style::NOP,
        );
    }

    Ok(())
}
