# The Cargo Book

![Cargo Logo](images/Cargo-Logo-Small.png)

Cargo is the [Rust] [*package manager*][def-package-manager]. Cargo downloads your Rust [package][def-package]'s
dependencies, compiles your packages, makes distributable packages, and uploads them to
[crates.io], the Rust community’s [*package registry*][def-package-registry]. You can contribute
to this book on [GitHub].

## Sections

**[Getting Started](getting-started/index.md)**

To get started with Cargo, install Cargo (and Rust) and set up your first
[*crate*][def-crate].

**[Cargo Guide](guide/index.md)**

The guide will give you all you need to know about how to use Cargo to develop
Rust packages.

**[Cargo Reference](reference/index.md)**

The reference covers the details of various areas of Cargo.

**[Cargo Commands](commands/index.md)**

The commands will let you interact with Cargo using its command-line interface.

**[Frequently Asked Questions](faq.md)**

**Appendices:**
* [Glossary](appendix/glossary.md)
* [Git Authentication](appendix/git-authentication.md)

**Other Documentation:**
* [Changelog](https://github.com/rust-lang/cargo/blob/master/CHANGELOG.md)
  --- Detailed notes about changes in Cargo in each release.
* [Rust documentation website](https://doc.rust-lang.org/) --- Links to official
  Rust documentation and tools.

[def-crate]:            ./appendix/glossary.md#crate            '"crate" (glossary entry)'
[def-package]:          ./appendix/glossary.md#package          '"package" (glossary entry)'
[def-package-manager]:  ./appendix/glossary.md#package-manager  '"package manager" (glossary entry)'
[def-package-registry]: ./appendix/glossary.md#package-registry '"package registry" (glossary entry)'
[rust]: https://www.rust-lang.org/
[crates.io]: https://crates.io/
[GitHub]: https://github.com/rust-lang/cargo/tree/master/src/doc
