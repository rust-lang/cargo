use anyhow::Context as _;
use cargo::util::command_prelude::{ArgMatchesExt, flag};
use cargo::util::lints::{Lint, LintLevel};
use itertools::Itertools;

use std::fmt::Write;
use std::path::PathBuf;

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
    for lint in cargo::util::lints::LINTS
        .iter()
        .sorted_by_key(|lint| lint.name)
    {
        if !lint.hidden {
            let sectipn = match lint.default_level {
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

    let docs_output_path = lint_docs_output_path();
    if check {
        let old = std::fs::read_to_string(docs_output_path)?;
        if old != buf {
            anyhow::bail!(
                "The lints documentation is out-of-date. Run `cargo lint-docs` to update it."
            );
        }
    } else {
        std::fs::write(docs_output_path, buf)?;
    }
    Ok(())
}

fn add_lint(lint: &Lint, buf: &mut String) -> anyhow::Result<()> {
    writeln!(buf, "## `{}`\n", lint.name)?;
    writeln!(buf, "Set to `{}` by default\n", lint.default_level)?;

    let src_path = lint_docs_src_path(lint);
    let docs = std::fs::read_to_string(&src_path)
        .with_context(|| format!("failed to read {}", src_path.display()))?;
    writeln!(buf, "{docs}\n",)?;

    Ok(())
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

fn lint_docs_output_path() -> PathBuf {
    let pkg_root = env!("CARGO_MANIFEST_DIR");
    let ws_root = PathBuf::from(format!("{pkg_root}/../.."));
    let path = {
        let path = ws_root.join("src/doc/src/reference/lints.md");
        path.canonicalize().unwrap_or(path)
    };
    path
}

/// Gets the markdown source of the lint documentation.
fn lint_docs_src_path(lint: &Lint) -> PathBuf {
    let pkg_root = env!("CARGO_MANIFEST_DIR");
    let ws_root = PathBuf::from(format!("{pkg_root}/../.."));
    let path = {
        let path = ws_root
            .join("src/cargo/util/lints")
            .join(lint.name)
            .with_extension("md");
        path.canonicalize().unwrap_or(path)
    };
    path
}
