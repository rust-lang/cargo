use std::fmt::Write;
use std::path::PathBuf;

use cargo::lints::Lint;
use cargo::lints::LintLevel;
use cargo::util::command_prelude::{ArgMatchesExt, flag};
use itertools::Itertools;

fn cli() -> clap::Command {
    clap::Command::new("xtask-lint-docs").arg(flag("check", "Check that the docs are up-to-date"))
}

fn main() -> anyhow::Result<()> {
    let args = cli().get_matches();
    let check = args.flag("check");

    let mut allow = Vec::new();
    let mut warn = Vec::new();
    let mut deny = Vec::new();
    let mut forbid = Vec::new();

    let mut lint_docs = String::new();
    for lint in cargo::lints::LINTS.iter().sorted_by_key(|lint| lint.name) {
        if lint.docs.is_some() {
            let sectipn = match lint.primary_group.default_level {
                LintLevel::Allow => &mut allow,
                LintLevel::Warn => &mut warn,
                LintLevel::Deny => &mut deny,
                LintLevel::Forbid => &mut forbid,
            };
            sectipn.push(lint.name);
            add_lint(lint, &mut lint_docs)?;
        }
    }

    let mut buf = String::new();
    writeln!(buf, "# Lints\n")?;
    writeln!(
        buf,
        "Note: [Cargo's linting system is unstable](unstable.md#lintscargo) and can only be used on nightly toolchains"
    )?;
    writeln!(buf)?;

    lint_groups(&mut buf)?;

    if !allow.is_empty() {
        add_level_section(LintLevel::Allow, &allow, &mut buf)?;
    }
    if !warn.is_empty() {
        add_level_section(LintLevel::Warn, &warn, &mut buf)?;
    }
    if !deny.is_empty() {
        add_level_section(LintLevel::Deny, &deny, &mut buf)?;
    }
    if !forbid.is_empty() {
        add_level_section(LintLevel::Forbid, &forbid, &mut buf)?;
    }

    buf.push_str(&lint_docs);

    if check {
        let old = std::fs::read_to_string(lint_docs_path())?;
        if old != buf {
            anyhow::bail!(
                "The lints documentation is out-of-date. Run `cargo lint-docs` to update it."
            );
        }
    } else {
        std::fs::write(lint_docs_path(), buf)?;
    }
    Ok(())
}

fn lint_groups(buf: &mut String) -> anyhow::Result<()> {
    let (max_name_len, max_desc_len) = cargo::lints::LINT_GROUPS.iter().filter(|g| !g.hidden).fold(
        (0, 0),
        |(max_name_len, max_desc_len), group| {
            // We add 9 to account for the "cargo::" prefix and backticks
            let name_len = group.name.chars().count() + 9;
            let desc_len = group.desc.chars().count();
            (max_name_len.max(name_len), max_desc_len.max(desc_len))
        },
    );
    let default_level_len = "Default level".chars().count();
    writeln!(buf, "\n")?;
    writeln!(
        buf,
        "| {:<max_name_len$} | {:<max_desc_len$} | Default level |",
        "Group", "Description",
    )?;
    writeln!(
        buf,
        "|-{}-|-{}-|-{}-|",
        "-".repeat(max_name_len),
        "-".repeat(max_desc_len),
        "-".repeat(default_level_len)
    )?;
    for group in cargo::lints::LINT_GROUPS.iter() {
        if group.hidden {
            continue;
        }
        let group_name = format!("`cargo::{}`", group.name);
        writeln!(
            buf,
            "| {:<max_name_len$} | {:<max_desc_len$} | {:<default_level_len$} |",
            group_name,
            group.desc,
            group.default_level.to_string(),
        )?;
    }
    writeln!(buf, "\n")?;
    Ok(())
}

fn add_lint(lint: &Lint, buf: &mut String) -> std::fmt::Result {
    writeln!(buf, "## `{}`", lint.name)?;
    writeln!(buf, "Group: `{}`\n", lint.primary_group.name)?;
    writeln!(buf, "Level: `{}`", lint.primary_group.default_level)?;
    writeln!(buf, "{}\n", lint.docs.as_ref().unwrap())
}

fn add_level_section(level: LintLevel, lint_names: &[&str], buf: &mut String) -> std::fmt::Result {
    let title = match level {
        LintLevel::Allow => "Allowed-by-default",
        LintLevel::Warn => "Warn-by-default",
        LintLevel::Deny => "Deny-by-default",
        LintLevel::Forbid => "Forbid-by-default",
    };
    writeln!(buf, "## {title}\n")?;
    writeln!(
        buf,
        "These lints are all set to the '{}' level by default.",
        level
    )?;

    for name in lint_names {
        writeln!(buf, "- [`{}`](#{})", name, name)?;
    }
    writeln!(buf)?;
    Ok(())
}

fn lint_docs_path() -> PathBuf {
    let pkg_root = env!("CARGO_MANIFEST_DIR");
    let ws_root = PathBuf::from(format!("{pkg_root}/../.."));
    let path = {
        let path = ws_root.join("src/doc/src/reference/lints.md");
        path.canonicalize().unwrap_or(path)
    };
    path
}
