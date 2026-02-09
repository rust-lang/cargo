use std::path::Path;

use annotate_snippets::Group;
use annotate_snippets::Level;
use annotate_snippets::Origin;
use annotate_snippets::Patch;
use annotate_snippets::Snippet;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Package;
use crate::core::Workspace;
use crate::lints::Lint;
use crate::lints::LintLevel;
use crate::lints::SUSPICIOUS;
use crate::lints::rel_cwd_manifest_path;

pub static LINT: Lint = Lint {
    name: "missing_lints_inheritance",
    desc: "missing `[lints]` to inherit `[workspace.lints]`",
    primary_group: &SUSPICIOUS,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks for packages without a `lints` table while `workspace.lints` is present.

### Why it is bad

Many people mistakenly think that `workspace.lints` is implicitly inherited when it is not.

### Drawbacks

### Example

```toml
[workspace.lints.cargo]
```

Should be written as:

```toml
[workspace.lints.cargo]

[lints]
workspace = true
```
"#,
    ),
};

pub fn missing_lints_inheritance(
    ws: &Workspace<'_>,
    pkg: &Package,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let (lint_level, reason) = LINT.level(
        cargo_lints,
        pkg.manifest().edition(),
        pkg.manifest().unstable_features(),
    );

    if lint_level == LintLevel::Allow {
        return Ok(());
    }

    let root = ws.root_maybe();
    // `normalized_toml` normally isn't guaranteed to include inheritance information except
    // `workspace.lints` is used outside of inheritance for workspace-level lints.
    let ws_lints = root
        .normalized_toml()
        .workspace
        .as_ref()
        .map(|ws| ws.lints.is_some())
        .unwrap_or(false);
    if !ws_lints {
        return Ok(());
    }
    if pkg.manifest().normalized_toml().lints.is_some() {
        return Ok(());
    }

    let manifest = pkg.manifest();
    let contents = manifest.contents();
    let level = lint_level.to_diagnostic_level();
    let emitted_source = LINT.emitted_source(lint_level, reason);
    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);

    let mut primary = Group::with_title(level.primary_title(LINT.desc));
    primary = primary.element(Origin::path(&manifest_path));
    primary = primary.element(Level::NOTE.message(emitted_source));
    let mut report = vec![primary];
    if let Some(contents) = contents {
        let span = contents.len()..contents.len();
        let mut help =
            Group::with_title(Level::HELP.secondary_title("to inherit `workspace.lints, add:"));
        help = help.element(
            Snippet::source(contents)
                .path(&manifest_path)
                .patch(Patch::new(span.clone(), "\n[lints]\nworkspace = true")),
        );
        report.push(help);
        let mut help = Group::with_title(
            Level::HELP.secondary_title("to clarify your intent to not inherit, add:"),
        );
        help = help.element(
            Snippet::source(contents)
                .path(&manifest_path)
                .patch(Patch::new(span, "\n[lints]")),
        );
        report.push(help);
    }

    if lint_level.is_error() {
        *error_count += 1;
    }
    gctx.shell().print_report(&report, lint_level.force())?;

    Ok(())
}
