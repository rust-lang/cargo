# Why Cargo Exists

## Preliminaries

In Rust, as you may know, a library or executable program is called a
[*crate*][def-crate]. Crates are compiled using the Rust compiler,
`rustc`. When starting with Rust, the first source code most people encounter
is that of the venerable “hello world” program, which they compile by invoking
`rustc` directly:

```console
$ rustc hello.rs
$ ./hello
Hello, world!
```

Note that the above command required that we specify the file name
explicitly. If we were to directly use `rustc` to compile a different program,
a different command line invocation would be required. If we needed to specify
any specific compiler flags or include external dependencies, then the
needed command would be even more specific (and elaborate).

Furthermore, most non-trivial programs will likely have dependencies on
external libraries, and will therefore also depend transitively on *their*
dependencies. Obtaining the correct versions of all the necessary dependencies
and keeping them up to date would be laborious and error-prone if done by
hand.

Rather than work only with crates and `rustc`, we can avoid the manual tedium
involved with performing the above tasks by introducing a higher-level
["*package*"][def-package] abstraction and by using a
[*package manager*][def-package-manager].

## Enter: Cargo

*Cargo* is the Rust package manager. It is a tool that allows Rust
[*packages*][def-package] to declare their various dependencies and ensure
that you’ll always get a repeatable build.

To accomplish this goal, Cargo does four things:

* Introduces two metadata files with various bits of package information.
* Fetches and builds your package’s dependencies.
* Invokes `rustc` or another build tool with the correct parameters to build
  your package.
* Introduces conventions to make working with Rust packages easier.

To a large extent, Cargo normalizes the commands needed to build a given
program or library; this is one aspect to the above mentioned conventions. As
we show later, the same command can be used to build different
[*artifacts*][def-artifact], regardless of their names. Rather than invoke
`rustc` directly, we can instead invoke something generic such as `cargo
build` and let cargo worry about constructing the correct `rustc`
invocation. Furthermore, Cargo will automatically fetch from a
[*registry*][def-registry] any dependencies we have defined for our artifact,
and arrange for them to be incorporated into our build as needed.

It is only a slight exaggeration to say that once you know how to build one
Cargo-based project, you know how to build *all* of them.

[def-artifact]:         ../appendix/glossary.md#artifact         '"artifact" (glossary entry)'
[def-crate]:            ../appendix/glossary.md#crate            '"crate" (glossary entry)'
[def-package]:          ../appendix/glossary.md#package          '"package" (glossary entry)'
[def-package-manager]:  ../appendix/glossary.md#package-manager  '"package manager" (glossary entry)'
[def-registry]:         ../appendix/glossary.md#registry         '"registry" (glossary entry)'
