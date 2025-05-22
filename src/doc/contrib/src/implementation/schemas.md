# Data Schemas

Cargo reads and writes user and machine facing data formats, like
- `Cargo.toml`, read and written on `cargo package`
- `Cargo.lock`, read and written
- `.cargo/config.toml`, read-only
- `cargo metadata` output
- `cargo build --message-format` output

## Schema Design

Generally,
- Fields should be kebab case
  - `#[serde(rename_all = "kebab-case")]` should be applied defensively
- Fields should only be present when needed, saving space and parse time
  - Also, we can always switch to always outputting the fields but its harder to stop outputting them
  - `#[serde(skip_serializing_if = "Default::default")]` should be applied liberally
- For output, prefer [jsonlines](https://jsonlines.org/) as it allows streaming output and flexibility to mix content (e.g. adding diagnostics to output that didn't previously have it
- `#[serde(deny_unknown_fields)]` should not be used to allow evolution of formats, including feature gating

## Schema Evolution Strategies

When changing a schema for data that is read, some options include:
- Adding new fields is relatively safe
  - If the field must not be ignored when present,
    have a transition period where it is invalid to use on stable Cargo before stabilizing it or
    error if its used before supported within the schema version
    (e.g. `edition` requires a minimum `package.rust-version`, if present)
- Adding new values to a field is relatively safe
  - Unstable values should fail on stable Cargo
- Version the structure and interpretation of the data (e.g. the `edition` field or `package.resolver` which has an `edition` fallback)

Note: some formats that are read are also written back out
(e.g. `cargo package` generating a `Cargo.toml` file)
and those strategies need to be considered as well.

When changing a schema for data that is written, some options include:
- Add new fields if the presence can be ignored
- Infer permission from the users use of the new schema (e.g. a new alias for an `enum` variant)
- Version the structure and interpretation of the format
  - Defaulting to the latest version with a warning that behavior may change (e.g. `cargo metadata --format-version`, `edition` in cargo script)
  - Defaulting to the first version, eventually warning the user of the implicit stale behavior (e.g. `package.edition` in `Cargo.toml`)
  - Without a default (e.g. `package.rust-version`, or a command-line flag like `--format-version`)

Note: While `serde` makes it easy to support data formats that add new fields,
new data types or supported values for a field are more difficult to future-proof
against.
