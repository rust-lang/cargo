### What it does

Checks if `hint-mostly-unused` being applied to all dependencies.

### Why it is bad

`hint-mostly-unused` indicates that most of a crate's API surface will go
unused by anything depending on it; this hint can speed up the build by
attempting to minimize compilation time for items that aren't used at all.
Misapplication to crates that don't fit that criteria will slow down the build
rather than speeding it up. It should be selectively applied to dependencies
that meet these criteria. Applying it globally is always a misapplication and
will likely slow down the build.

### Example

```toml
[profile.dev.package."*"]
hint-mostly-unused = true
```

Should instead be:

```toml
[profile.dev.package.huge-mostly-unused-dependency]
hint-mostly-unused = true
```
