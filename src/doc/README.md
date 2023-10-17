# Cargo documentation

This directory contains Cargo's documentation. There are two parts, [The Cargo
Book] which is built with [mdbook] and the man pages, which are built with
[mdman].

[The Cargo Book]: https://doc.rust-lang.org/cargo/
[mdBook]: https://github.com/rust-lang/mdBook
[mdman]: https://github.com/rust-lang/cargo/tree/master/crates/mdman/

### Building the book

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

The book contents are driven by the [`SUMMARY.md`](src/SUMMARY.md) file, and
every file must be linked there.

### Building the man pages

The man pages use a tool called [mdman] to convert markdown to a man page
format. Check out the documentation at
[`mdman/doc/`](../../crates/mdman/doc/)
for more details.

The man pages are converted from a templated markdown (located in the
[`src/doc/man/`](man)
directory) to three different formats:

1. Troff-style man pages, saved in [`src/etc/man/`](../etc/man).
2. Markdown (with some HTML) for the Cargo Book, saved in
   [`src/doc/src/commands/`](src/commands).
3. Plain text (needed for embedded man pages on platforms without man such as
   Windows), saved in [`src/doc/man/generated_txt/`](man/generated_txt).

To rebuild the man pages, run `cargo build-man` inside the workspace.

### SemVer chapter tests

There is a script to verify that the examples in the SemVer chapter work as
intended. To run the tests, run `cargo +stable run -p semver-check`.

Note that these tests run on the most recent stable release because they
validate the output of the compiler diagnostics. The output can change between
releases, so we pin to a specific release to avoid frequent and unexpected
breakage.

## Contributing

We'd love your help with improving the documentation! Please feel free to
[open issues](https://github.com/rust-lang/cargo/issues) about anything, and
send in PRs for things you'd like to fix or change. If your change is large,
please open an issue first, so we can make sure that it's something we'd
accept before you go through the work of getting a PR together.
