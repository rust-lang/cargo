### What it does

Checks for unknown lints in the `[lints.cargo]` table

### Why it is bad

- The lint name could be misspelled, leading to confusion as to why it is
  not working as expected
- The unknown lint could end up causing an error if `cargo` decides to make
  a lint with the same name in the future

### Example

```toml
[lints.cargo]
this-lint-does-not-exist = "warn"
```
