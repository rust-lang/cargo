# Cargo.toml vs Cargo.lock

`Cargo.toml` and `Cargo.lock` serve two different purposes. Before we talk
about them, here’s a summary:

* `Cargo.toml` is about describing your dependencies in a broad sense, and is
  written by you.
* `Cargo.lock` contains exact information about your dependencies. It is
  maintained by Cargo and should not be manually edited.

When in doubt, check `Cargo.lock` into the version control system (e.g. Git).
For a better understanding of why and what the alternatives might be, see
[“Why have Cargo.lock in version control?” in the FAQ](../faq.md#why-have-cargolock-in-version-control).
We recommend pairing this with
[Verifying Latest Dependencies](continuous-integration.md#verifying-latest-dependencies)

Let’s dig in a little bit more.

`Cargo.toml` is a [**manifest**][def-manifest] file in which we can specify a
bunch of different metadata about our package. For example, we can say that we
depend on another package:

```toml
[package]
name = "hello_world"
version = "0.1.0"

[dependencies]
regex = { git = "https://github.com/rust-lang/regex.git" }
```

This package has a single dependency, on the `regex` library. We’ve stated in
this case that we’re relying on a particular Git repository that lives on
GitHub. Since we haven’t specified any other information, Cargo assumes that
we intend to use the latest commit on the default branch to build our package.

Sound good? Well, there’s one problem: If you build this package today, and
then you send a copy to me, and I build this package tomorrow, something bad
could happen. There could be more commits to `regex` in the meantime, and my
build would include new commits while yours would not. Therefore, we would
get different builds. This would be bad because we want reproducible builds.

We could fix this problem by defining a specific `rev` value in our `Cargo.toml`,
so Cargo could know exactly which revision to use when building the package:

```toml
[dependencies]
regex = { git = "https://github.com/rust-lang/regex.git", rev = "9f9f693" }
```

Now our builds will be the same. But there’s a big drawback: now we have to
manually think about SHA-1s every time we want to update our library. This is
both tedious and error prone.

Enter the `Cargo.lock`. Because of its existence, we don’t need to manually
keep track of the exact revisions: Cargo will do it for us. When we have a
manifest like this:

```toml
[package]
name = "hello_world"
version = "0.1.0"

[dependencies]
regex = { git = "https://github.com/rust-lang/regex.git" }
```

Cargo will take the latest commit and write that information out into our
`Cargo.lock` when we build for the first time. That file will look like this:

```toml
[[package]]
name = "hello_world"
version = "0.1.0"
dependencies = [
 "regex 1.5.0 (git+https://github.com/rust-lang/regex.git#9f9f693768c584971a4d53bc3c586c33ed3a6831)",
]

[[package]]
name = "regex"
version = "1.5.0"
source = "git+https://github.com/rust-lang/regex.git#9f9f693768c584971a4d53bc3c586c33ed3a6831"
```

You can see that there’s a lot more information here, including the exact
revision we used to build. Now when you give your package to someone else,
they’ll use the exact same SHA, even though we didn’t specify it in our
`Cargo.toml`.

When we’re ready to opt in to a new version of the library, Cargo can
re-calculate the dependencies and update things for us:

```console
$ cargo update         # updates all dependencies
$ cargo update regex   # updates just “regex”
```

This will write out a new `Cargo.lock` with the new version information. Note
that the argument to `cargo update` is actually a
[Package ID Specification](../reference/pkgid-spec.md) and `regex` is just a
short specification.

[def-manifest]:  ../appendix/glossary.md#manifest  '"manifest" (glossary entry)'
[def-package]:   ../appendix/glossary.md#package   '"package" (glossary entry)'
