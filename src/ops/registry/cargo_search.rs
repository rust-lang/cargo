//! Interacts with the registry [search API][1].
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/registry-web-api.html#search

use crate::sources::CRATES_IO_DOMAIN;
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

    let links = crates
        .iter()
        .map(|krate| {
            registry
                .host_is_crates_io()
                .then(|| {
                    shell.out_hyperlink(format!(
                        "https://{CRATES_IO_DOMAIN}/crates/{}/{}",
                        krate.name, krate.max_version
                    ))
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();

    let stdout = shell.out();
    let good = style::GOOD;

    let write_with_highlighting = |stdout: &mut dyn std::io::Write, text: &str| {
        let mut fragments = text.split(query).peekable();
        while let Some(fragment) = fragments.next() {
            let _ = write!(stdout, "{fragment}");
            if fragments.peek().is_some() {
                let _ = write!(stdout, "{good}{query}{good:#}");
            }
        }
    };

    for (name, (description, link)) in names.into_iter().zip(descriptions.zip(links)) {
        // name with hyperlink
        let _ = write!(stdout, "{link}");
        write_with_highlighting(stdout, &name);
        let _ = write!(stdout, "{link:#}");

        if let Some(description) = description {
            let margin = " ".repeat(description_margin.saturating_sub(name.len()));
            let _ = write!(stdout, "{margin}# ");
            write_with_highlighting(stdout, &description);
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
