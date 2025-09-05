# Documentation

Cargo has several types of documentation that contributors work with:

* [The Cargo Book]
  * The primary user-facing Cargo documentation
  * Source at <https://github.com/rust-lang/cargo/tree/master/src/doc>
  * Built with [mdbook]
  * Published through ["the doc publishing process"]
* Man pages
  * Man pages of the `cargo` command
  * Built with [mdman]
  * Published through ["the doc publishing process"]
* [Contributor guide]
  * This guide itself
  * Source at <https://github.com/rust-lang/cargo/tree/master/src/doc/contrib>
  * Published independently on GitHub Pages at
    <https://rust-lang.github.io/cargo/contrib>
    when committing to the master branch.

[The Cargo Book]: https://doc.rust-lang.org/cargo/
[Crate API docs]: https://docs.rs/cargo
[Contributor guide]: https://rust-lang.github.io/cargo/contrib
[mdBook]: https://github.com/rust-lang/mdBook
[mdman]: https://github.com/rust-lang/cargo/tree/master/crates/mdman/
["the doc publishing process"]: ../process/release.md#docs-publishing

## Building the book

Building the book requires [mdBook]. To get it:

```console
$ cargo install mdbook
```

To build the book:

```console
$ mdbook build
```

`mdbook` provides a variety of different commands and options to help you work
on the book:

* `mdbook build --open`: Build the book and open it in a web browser.
* `mdbook serve`: Launches a web server on localhost. It also automatically
  rebuilds the book whenever any file changes and automatically reloads your
  web browser.

The book contents are driven by a `SUMMARY.md` file,
and every file must be linked there.
See <https://rust-lang.github.io/mdBook/> for its usage.

## Building the man pages

The man pages use a tool called [mdman] to convert Markdown templates into several output formats.
See <https://github.com/rust-lang/cargo/tree/master/crates/mdman/doc>
for usage details and template syntax.

The templates are located in
<https://github.com/rust-lang/cargo/tree/master/src/doc/man>
and are converted into three formats:

1. Troff man pages --- used by the `cargo help` command,
   and by distributions to provide man pages which people can install,
   saved in <https://github.com/rust-lang/cargo/tree/master/src/etc/man>.
2. Plain text --- used for embedded help on platforms without `man` (such as Windows),
   saved in <https://github.com/rust-lang/cargo/tree/master/src/doc/man/generated_txt>.
3. Markdown (with some HTML) --- used for the Cargo Book,
   saved in <https://github.com/rust-lang/cargo/tree/master/src/doc/src/commands>.

To rebuild the man pages, run `cargo build-man` inside the workspace.

## Writing guidelines

Cargo's documentation is a collective effort,
so there isn't a single fixed writing style.
We recommend following the style of the surrounding text to keep things consistent.

A few important guidelines:

* The [Cargo Commands](https://doc.rust-lang.org/nightly/cargo/commands/index.html)
  chapters in the book are generated from man page templates.
  To update them, see the [Building the man pages](#building-the-man-pages) section.
  Do not edit the generated Markdown files directly.
* Links to pages under <https://doc.rust-lang.org/> should use relative paths.
  This ensures versioned docs are redirected correctly.
  For example, if you are at <https://doc.rust-lang.org/cargo/reference/config.html>
  and want to link to <https://doc.rust-lang.org/rustc/codegen-options/index.html>,
  you should write the link as `../../rustc/codegen-options/index.html`.
  This rule doesn't apply if you specifically want to link to docs of a fixed version or channel.
* When renaming or removing any headings or pages,
  make sure to set up proper redirects via the [`output.html.redirect`] mdbook option.
* If a section refers to a concept explained elsewhere
  (like profiles, features, or workspaces), link to it.
  That keeps the book navigable without duplicating content.

[[`output.html.redirect`]]: https://rust-lang.github.io/mdBook/format/configuration/renderers.html#outputhtmlredirect

## SemVer chapter tests

There is a script to verify that the examples in [the SemVer chapter] work as
intended. To run the tests, run `cargo +stable run -p semver-check`.

Note that these tests run on the most recent stable release because they
validate the output of the compiler diagnostics. The output can change between
releases, so we pin to a specific release to avoid frequent and unexpected
breakage.

[the SemVer chapter]: https://doc.rust-lang.org/nightly/cargo/reference/semver.html
