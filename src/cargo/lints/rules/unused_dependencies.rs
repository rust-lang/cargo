use crate::lints::Lint;
use crate::lints::STYLE;

pub static LINT: &Lint = &Lint {
    name: "unused_dependencies",
    desc: "unused dependency",
    primary_group: &STYLE,
    msrv: Some(super::CARGO_LINTS_MSRV),
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks for dependencies that are not used by any of the cargo targets.

### Why it is bad

Slows down compilation time.

### Drawbacks

The lint is only emitted in specific circumstances as multiple cargo targets exist for the
different dependencies tables and they must all be built to know if a dependency is unused.
Currently, only the selected packages are checked and not all `path` dependencies like most lints.
The cargo target selection flags,
independent of which packages are selected, determine which dependencies tables are checked.
As there is no way to select all cargo targets that use `[dev-dependencies]`,
they are unchecked.

Examples:
- `cargo check` will lint `[build-dependencies]` and `[dependencies]`
- `cargo check --all-targets` will still only lint `[build-dependencies]` and `[dependencies]` and not `[dev-dependencoes]`
- `cargo check --bin foo` will not lint `[dependencies]` even if `foo` is the only bin though `[build-dependencies]` will be checked
- `cargo check -p foo` will not lint any dependencies tables for the `path` dependency `bar` even if `bar` only has a `[lib]`

### Example

```toml
[package]
name = "foo"

[dependencies]
unused = "1"
```

Should be written as:

```toml
[package]
name = "foo"
```
"#,
    ),
};
