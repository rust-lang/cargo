use crate::lints::Lint;
use crate::lints::STYLE;

pub const LINT: Lint = Lint {
    name: "unused_dependencies",
    desc: "unused dependency",
    primary_group: &STYLE,
    edition_lint_opts: None,
    feature_gate: None,
    docs: Some(
        r#"
### What it does

Checks for dependencies that are not used by any of the cargo targets.

### Why it is bad

Slows down compilation time.

### Drawbacks

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
