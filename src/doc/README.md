# Cargo documentation

This directory contains Cargo's documentation. There are two parts, [The Cargo Book]
which is built with [mdbook] and the man pages, which are built with [Asciidoctor].
The man pages are also included in The Cargo Book as HTML.

[The Cargo Book]: https://doc.rust-lang.org/cargo/

### Building the book

Building the book requires [mdBook]. To get it:

[mdBook]: https://github.com/rust-lang/mdBook

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

Building the man pages requires [Asciidoctor]. See the linked page for
installation instructions. It also requires the `make` build tool and `ruby`.

[Asciidoctor]: https://asciidoctor.org/

The source files are located in the [`src/doc/man`](man) directory. The
[`Makefile`](Makefile) is used to rebuild the man pages. It outputs the man
pages into [`src/etc/man`](../etc/man) and the HTML versions into
[`src/doc/man/generated`](man/generated). The Cargo Book has some markdown
stub files in [`src/doc/src/commands`](src/commands) which load the generated
HTML files.

To build the man pages, run `make` in the `src/doc` directory:

```console
$ make
```

The build script uses a few Asciidoctor extensions. See
[`asciidoc-extension.rb`](asciidoc-extension.rb) for details.

## Contributing

We'd love your help with improving the documentation! Please feel free to
[open issues](https://github.com/rust-lang/cargo/issues) about anything, and
send in PRs for things you'd like to fix or change. If your change is large,
please open an issue first, so we can make sure that it's something we'd
accept before you go through the work of getting a PR together.
