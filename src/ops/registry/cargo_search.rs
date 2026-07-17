//! Interacts with the registry [search API][1].
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#search

use std::cmp;

use anyhow::Context as _;
use url::Url;

use crate::CargoResult;
use crate::GlobalContext;
use crate::util::style;
use crate::util::style::LITERAL;
use crate::util::truncate_with_ellipsis;

use super::RegistryOrIndex;

pub fn search(
    query: &str,
    gctx: &GlobalContext,
    reg_or_index: Option<RegistryOrIndex>,
    limit: u32,
) -> CargoResult<()> {
    let source_ids = super::get_source_id(gctx, reg_or_index.as_ref())?;
    let (mut registry, _) =
        super::registry(gctx, &source_ids, None, reg_or_index.as_ref(), false, None)?;
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

    let description_margin = names.iter().map(|s| s.len()).max().unwrap_or_default() + 4;

    let description_length = cmp::max(80, 128 - description_margin);

    let descriptions = crates.iter().map(|krate| {
        krate
            .description
            .as_ref()
            .map(|desc| truncate_with_ellipsis(&desc.replace("\n", " "), description_length))
    });

    let mut shell = gctx.shell();
    let stdout = shell.out();
    let good = style::GOOD;

    for (name, description) in names.into_iter().zip(descriptions) {
        let line = match description {
            Some(desc) => format!("{name: <description_margin$}# {desc}"),
            None => name,
        };
        let mut fragments = line.split(query).peekable();
        while let Some(fragment) = fragments.next() {
            let _ = write!(stdout, "{fragment}");
            if fragments.peek().is_some() {
                let _ = write!(stdout, "{good}{query}{good:#}");
            }
        }
        let _ = writeln!(stdout);
    }

    let search_max_limit = 100;
    if total_crates > limit && limit < search_max_limit {
        let _ = writeln!(
            stdout,
            "... and {} crates more (use --limit N to see more)",
            total_crates - limit
        );
    } else if total_crates > limit && limit >= search_max_limit {
        let extra = if source_ids.original.is_crates_io() {
            let url = Url::parse_with_params("https://crates.io/search", &[("q", query)])?;
            format!(" (go to {url} to see more)")
        } else {
            String::new()
        };
        let _ = writeln!(
            stdout,
            "... and {} crates more{}",
            total_crates - limit,
            extra
        );
    }

    if total_crates > 0 {
        let literal = LITERAL;
        shell.note(format_args!(
            "to learn more about a package, run `{literal}cargo info <name>{literal:#}`",
        ))?;
    }

    Ok(())
}
