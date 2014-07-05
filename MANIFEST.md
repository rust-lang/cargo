The manifest file (`Cargo.toml`) is still under active development. This document captures the current thinking on the design.

## Sections

The `Cargo.toml` file contains several top-level sections:

* `[project]`: project-specific details, such as name, version and author
* `[[lib]]`: information about the main library file, if one exists. By
  default, the main library file is `src/<package-name>.rs`
* `[[bin]]`: optionally repeated information about executables
  that the project is generating. This can both be used for projects
  that primarily build executables, as well as projects that contain
  utility executables (such as an HTTP library that comes with a web
  server executable).

## Extensibility

In general, any unknown attributes are ignored for future extensibility.
Future plugins may also use this capability.

The project uses the `Decoder` in `rust-toml` to decode the manifest
into Rust structs that are used throughout the built-in commands.

## The `[project]` Section

* `name`: the name of the project (`String`)
* `version`: the version of the project, (`String` that can be parsed
  via `semver::parse`)
* `readme`: a Markdown-formatted file in the project that can be used as
  a description of the document in indexes (`Option<Path>`, relative to
  the project root, defaults to "./README.md", if found).
* `tags`: an array of tags that can be used in indexes (`Vec<String>`)
* `authors`: a list of authors in `name <email>` format (`Vec<String>`). At
  least one `author` with email will probably be required to submit to
  the Cargo repository.
* `src`: the root directory containing source files (`Option<Path>`,
  relative to the project root, defaults to `src`)

## The `[[lib]]` Section

At the moment, Cargo only supports a single lib per package. We use the
array format for future extensibility.

We only plan to support a single lib at the moment because if you have
multiple libs, you would want projects to be able to depend on them
separately. If you don't care about that, why do you have separate libs?

* `name`: the name of the library (`String`, `hammer` would create a `libhammer`)
* `path`: the location of the main crate file (`Option<Path>`, defaults to
  `src/<name>.rs`)

Note that we plan to support multiple `Cargo.toml` files in Cargo's git
support, so you don't have to have a separate git repository per
library.

## The `[[bin]]` Section

The `bin` section is optionally repeated. It is designed for
projects whose main raison d'être is a single executable, or for projects
that want to provide utility executables alongside a primary library.

If an executable has a different set of flags or dependencies from the
main library, it should be shipped as a separate package with a
dependency on the main library to keep the usage requirements of the
standalone library limited to the bare minimum requirements.

* `name`: the name of the executable (`String`, `hammer` would create a
  `hammer` executable)
* `path`: the location of the main crate file for the executable
  (`Option<Path>`, defaults to `src/<name>.rs` if the project has only
  an executable, `src/bin/<name>.rs` if the project has both a lib and
  executable, see below)

## The `[dependencies]` Section

```toml
[dependencies]

rust-http = "1.x"
hammer = ["> 1.2", "< 1.3.5"]

[dependencies.hamcrest]

version = "1.2.x"
git = "http://github.com/carllerche/hamcrest"
```

## Projects Containing Both `lib` and `executable`

Most projects will primarily produce either a library or an executable.
Such projects should name the main crate file `<projectname>.rs` and
omit the optional `path` from the `lib` or `executable` sections.

Projects that contain both a library and one or more executables should
generally use `<projectname>.rs` for their library, and `bin/*.rs`
for the executables.

These rules are also the default behavior if `path` is left off of `lib`
or `executable` sections.

## Example Manifests

Simple `lib`:

```toml
[project]

name = "hammer"
version = "0.1.0"
readme = "README.md"
authors = ["Yehuda Katz <wycats@gmail.com>"]

[[lib]]

name = "hammer"
```

Simple `executable`:

```toml
[project]

name = "skylight"
version = "0.5.0"
authors = ["Tom Dale <tom@tomdale.net>", "Carl Lerche <me@carllerche.com>"]
tags = ["performance", "profiling"]

[[executable]]

name = "skylight"
path = "bin/skylight.rs" # supports existing project structures
```

A project with both a lib and an `executable`:

```toml
[project]

name = "skylight"
version = "0.5.0"
authors = ["Tom Dale <tom@tomdale.net>", "Carl Lerche <me@carllerche.com>"]
tags = ["performance", "profiling"]

[[lib]]

name = "skylight" # path defaults to src/skylight.rs

[[executable]]

name = "skylight" # path defaults to src/bin/skylight.rs

[[executable]]

name = "skylight-setup" # path defaults to src/bin/skylight-setup.rs
```
