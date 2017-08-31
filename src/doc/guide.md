% Cargo Guide

Welcome to the Cargo guide. This guide will give you all that you need to know
about how to use Cargo to develop Rust projects.

# Why Cargo exists

Cargo is a tool that allows Rust projects to declare their various
dependencies and ensure that you’ll always get a repeatable build.

To accomplish this goal, Cargo does four things:

* Introduces two metadata files with various bits of project information.
* Fetches and builds your project’s dependencies.
* Invokes `rustc` or another build tool with the correct parameters to build
  your project.
* Introduces conventions to make working with Rust projects easier.

# Creating a new project

To start a new project with Cargo, use `cargo new`:

```shell
$ cargo new hello_world --bin
```

We’re passing `--bin` because we’re making a binary program: if we
were making a library, we’d leave it off. This also initializes a new `git`
repository by default. If you don't want it to do that, pass `--vcs none`.

Let’s check out what Cargo has generated for us:

```shell
$ cd hello_world
$ tree .
.
├── Cargo.toml
└── src
    └── main.rs

1 directory, 2 files
```

If we had just used `cargo new hello_world` without the `--bin` flag, then
we would have a `lib.rs` instead of a `main.rs`. For now, however, this is all
we need to get started. First, let’s check out `Cargo.toml`:

```toml
[package]
name = "hello_world"
version = "0.1.0"
authors = ["Your Name <you@example.com>"]
```

This is called a **manifest**, and it contains all of the metadata that Cargo
needs to compile your project.

Here’s what’s in `src/main.rs`:

```
fn main() {
    println!("Hello, world!");
}
```

Cargo generated a “hello world” for us. Let’s compile it:

```shell
$ cargo build
   Compiling hello_world v0.1.0 (file:///path/to/project/hello_world)
```

And then run it:

```shell
$ ./target/debug/hello_world
Hello, world!
```

We can also use `cargo run` to compile and then run it, all in one step (You
won't see the `Compiling` line if you have not made any changes since you last
compiled):

```shell
$ cargo run
   Compiling hello_world v0.1.0 (file:///path/to/project/hello_world)
     Running `target/debug/hello_world`
Hello, world!
```

You’ll now notice a new file, `Cargo.lock`. It contains information about our
dependencies. Since we don’t have any yet, it’s not very interesting.

Once you’re ready for release, you can use `cargo build --release` to compile
your files with optimizations turned on:

```shell
$ cargo build --release
   Compiling hello_world v0.1.0 (file:///path/to/project/hello_world)
```

`cargo build --release` puts the resulting binary in `target/release` instead of
`target/debug`.

Compiling in debug mode is the default for development-- compilation time is
shorter since the compiler doesn't do optimizations, but the code will run
slower. Release mode takes longer to compile, but the code will run faster.

# Working on an existing Cargo project

If you download an existing project that uses Cargo, it’s really easy
to get going.

First, get the project from somewhere. In this example, we’ll use `rand`
cloned from its repository on GitHub:

```shell
$ git clone https://github.com/rust-lang-nursery/rand.git
$ cd rand
```

To build, use `cargo build`:

```shell
$ cargo build
   Compiling rand v0.1.0 (file:///path/to/project/rand)
```

This will fetch all of the dependencies and then build them, along with the
project.

# Adding dependencies from crates.io

[crates.io] is the Rust community's central package registry that serves as a
location to discover and download packages. `cargo` is configured to use it by
default to find requested packages.

To depend on a library hosted on [crates.io], add it to your `Cargo.toml`.

[crates.io]: https://crates.io/

## Adding a dependency

If your `Cargo.toml` doesn't already have a `[dependencies]` section, add that,
then list the crate name and version that you would like to use. This example
adds a dependency of the `time` crate:

```toml
[dependencies]
time = "0.1.12"
```

The version string is a [semver] version requirement. The [specifying
dependencies](specifying-dependencies.html) docs have more information about
the options you have here.

[semver]: https://github.com/steveklabnik/semver#requirements

If we also wanted to add a dependency on the `regex` crate, we would not need
to add `[dependencies]` for each crate listed. Here's what your whole
`Cargo.toml` file would look like with dependencies on the `time` and `regex`
crates:

```toml
[package]
name = "hello_world"
version = "0.1.0"
authors = ["Your Name <you@example.com>"]

[dependencies]
time = "0.1.12"
regex = "0.1.41"
```

Re-run `cargo build`, and Cargo will fetch the new dependencies and all of
their dependencies, compile them all, and update the `Cargo.lock`:

```shell
$ cargo build
      Updating registry `https://github.com/rust-lang/crates.io-index`
   Downloading memchr v0.1.5
   Downloading libc v0.1.10
   Downloading regex-syntax v0.2.1
   Downloading memchr v0.1.5
   Downloading aho-corasick v0.3.0
   Downloading regex v0.1.41
     Compiling memchr v0.1.5
     Compiling libc v0.1.10
     Compiling regex-syntax v0.2.1
     Compiling memchr v0.1.5
     Compiling aho-corasick v0.3.0
     Compiling regex v0.1.41
     Compiling hello_world v0.1.0 (file:///path/to/project/hello_world)
```

Our `Cargo.lock` contains the exact information about which revision of all of
these dependencies we used.

Now, if `regex` gets updated, we will still build with the same revision until
we choose to `cargo update`.

You can now use the `regex` library using `extern crate` in `main.rs`.

```
extern crate regex;

use regex::Regex;

fn main() {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    println!("Did our date match? {}", re.is_match("2014-01-01"));
}
```

Running it will show:

```shell
$ cargo run
   Running `target/hello_world`
Did our date match? true
```
# Project layout

Cargo uses conventions for file placement to make it easy to dive into a new
Cargo project:

```shell
.
├── Cargo.lock
├── Cargo.toml
├── benches
│   └── large-input.rs
├── examples
│   └── simple.rs
├── src
│   ├── bin
│   │   └── another_executable.rs
│   ├── lib.rs
│   └── main.rs
└── tests
    └── some-integration-tests.rs
```

* `Cargo.toml` and `Cargo.lock` are stored in the root of your project (*package
  root*).
* Source code goes in the `src` directory.
* The default library file is `src/lib.rs`.
* The default executable file is `src/main.rs`.
* Other executables can be placed in `src/bin/*.rs`.
* Integration tests go in the `tests` directory (unit tests go in each file
  they're testing).
* Examples go in the `examples` directory.
* Benchmarks go in the `benches` directory.

These are explained in more detail in the [manifest
description](manifest.html#the-project-layout).

# Cargo.toml vs Cargo.lock

`Cargo.toml` and `Cargo.lock` serve two different purposes. Before we talk
about them, here’s a summary:

* `Cargo.toml` is about describing your dependencies in a broad sense, and is
  written by you.
* `Cargo.lock` contains exact information about your dependencies. It is
  maintained by Cargo and should not be manually edited.

If you’re building a library that other projects will depend on, put
`Cargo.lock` in your `.gitignore`. If you’re building an executable like a
command-line tool or an application, check `Cargo.lock` into `git`. If you're
curious about why that is, see ["Why do binaries have `Cargo.lock` in version
control, but not libraries?" in the
FAQ](faq.html#why-do-binaries-have-cargolock-in-version-control-but-not-libraries).

Let’s dig in a little bit more.

`Cargo.toml` is a **manifest** file in which we can specify a bunch of
different metadata about our project. For example, we can say that we depend
on another project:

```toml
[package]
name = "hello_world"
version = "0.1.0"
authors = ["Your Name <you@example.com>"]

[dependencies]
rand = { git = "https://github.com/rust-lang-nursery/rand.git" }
```

This project has a single dependency, on the `rand` library. We’ve stated in
this case that we’re relying on a particular Git repository that lives on
GitHub. Since we haven’t specified any other information, Cargo assumes that
we intend to use the latest commit on the `master` branch to build our project.

Sound good? Well, there’s one problem: If you build this project today, and
then you send a copy to me, and I build this project tomorrow, something bad
could happen. There could be more commits to `rand` in the meantime, and my
build would include new commits while yours would not. Therefore, we would
get different builds. This would be bad because we want reproducible builds.

We could fix this problem by putting a `rev` line in our `Cargo.toml`:

```toml
[dependencies]
rand = { git = "https://github.com/rust-lang-nursery/rand.git", rev = "9f35b8e" }
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
authors = ["Your Name <you@example.com>"]

[dependencies]
rand = { git = "https://github.com/rust-lang-nursery/rand.git" }
```

Cargo will take the latest commit and write that information out into our
`Cargo.lock` when we build for the first time. That file will look like this:

```toml
[root]
name = "hello_world"
version = "0.1.0"
dependencies = [
 "rand 0.1.0 (git+https://github.com/rust-lang-nursery/rand.git#9f35b8e439eeedd60b9414c58f389bdc6a3284f9)",
]

[[package]]
name = "rand"
version = "0.1.0"
source = "git+https://github.com/rust-lang-nursery/rand.git#9f35b8e439eeedd60b9414c58f389bdc6a3284f9"

```

You can see that there’s a lot more information here, including the exact
revision we used to build. Now when you give your project to someone else,
they’ll use the exact same SHA, even though we didn’t specify it in our
`Cargo.toml`.

When we’re ready to opt in to a new version of the library, Cargo can
re-calculate the dependencies and update things for us:

```shell
$ cargo update           # updates all dependencies
$ cargo update -p rand   # updates just “rand”
```

This will write out a new `Cargo.lock` with the new version information. Note
that the argument to `cargo update` is actually a
[Package ID Specification](pkgid-spec.html) and `rand` is just a short
specification.

# Tests

Cargo can run your tests with the `cargo test` command. Cargo looks for tests
to run in two places: in each of your `src` files and any tests in `tests/`.
Tests in your `src` files should be unit tests, and tests in `tests/` should be
integration-style tests. As such, you’ll need to import your crates into
the files in `tests`.

Here's an example of running `cargo test` in our project, which currently has
no tests:

```shell
$ cargo test
   Compiling rand v0.1.0 (https://github.com/rust-lang-nursery/rand.git#9f35b8e)
   Compiling hello_world v0.1.0 (file:///path/to/project/hello_world)
     Running target/test/hello_world-9c2b65bbb79eabce

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

If our project had tests, we would see more output with the correct number of
tests.

You can also run a specific test by passing a filter:

```shell
$ cargo test foo
```

This will run any test with `foo` in its name.

`cargo test` runs additional checks as well. For example, it will compile any
examples you’ve included and will also test the examples in your
documentation. Please see the [testing guide][testing] in the Rust
documentation for more details.

[testing]: https://doc.rust-lang.org/book/testing.html

## Travis CI

To test your project on Travis CI, here is a sample `.travis.yml` file:

```yaml
language: rust
rust:
  - stable
  - beta
  - nightly
matrix:
  allow_failures:
    - rust: nightly
```

This will test all three release channels, but any breakage in nightly
will not fail your overall build. Please see the [Travis CI Rust
documentation](https://docs.travis-ci.com/user/languages/rust/) for more
information.

# Further reading

Now that you have an overview of how to use cargo and have created your first
crate, you may be interested in:

* [Publishing your crate on crates.io](crates-io.html)
* [Reading about all the possible ways of specifying dependencies](specifying-dependencies.html)
* [Learning more details about what you can specify in your `Cargo.toml` manifest](manifest.html)

Even more topics are available in the Docs menu at the top!
