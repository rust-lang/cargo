---
title: Building external code
---

Some packages need to compile third-party non-Rust code that you will
link into your Rust code using `#[link]` (more information on `#[link]`
can be found in [the Rust manual][1]).

Cargo does not aim to replace other tools that are well-optimized for
building C or C++ code, but it does integrate with them with the `build`
configuration option.

```toml
[package]

name = "hello-world-from-c"
version = "0.1.0"
authors = [ "wycats@gmail.com" ]
build = "make"
```

The `build` command will be invoked before `rustc`, allowing your Rust
code to depend on the built artifacts.

Here's what you need to know:

* Cargo passes your build script an environment variable named
  `OUT_DIR`, which is where you should put any compiled artifacts. It
  will be different for different Cargo commands, but Cargo will always
  pass that output directory as a lib directory to `rustc`.
* Cargo will retain all output in `OUT_DIR` for clean packages across
  builds (intelligently discarding the compiled artifacts for dirty
  dependencies). Do not put the output of a build command in any other
  directory.
* The actual location of `$OUT_DIR` is
  `/path/to/project/target/native/$your-out-dir`.
* The target triple that the build command should compile for is specified by
  the `TARGET` environment variable.

What this means is that the normal workflow for build dependencies is:

* The first time a user types `cargo build` for a project that contains
  your package, your `build` script will be invoked. Place any artifacts
  into the provided `$OUT_DIR`.
* The next time a user runs `cargo build`, if the dependency has not
  changed (via `cargo update <your-package>`), Cargo will reuse the
  output you provided before. Your build command will not be invoked.
* If the user updates your package to a new version (or git revision),
  Cargo will **not** remove the old `$OUT_DIR` will re-invoke your build script.
  Your build script is responsible for bringing the state of the old directory
  up to date with the current state of the input files.

In general, build scripts may not be as portable as we'd like today. We
encourage package authors to write build scripts that can work in both
Windows and Unix environments.

Several people who work on Cargo are also working on a project called
[link-config][2], which is a Rust syntax extension whose goal is to
enable portable external compilation and linkage against system
packages. We intend for it to eventually serve this purpose for Cargo
projects.

[1]: http://doc.rust-lang.org/rust.html#linkage
[2]: https://github.com/alexcrichton/link-config

# Environment Variables

The following environment variables are always available for build
commands.

* `OUT_DIR` - the folder in which all output should be placed.
* `TARGET` - the target triple that is being compiled for. Native code should be
             compiled for this triple.
* `DEP_<name>_OUT_DIR` - This variable is present for all immediate dependencies
                         of the package being built. The `<name>` will be the
                         package's name, in uppercase, with `-` characters
                         translated to a `_`. The value of this variable is the
                         directory in which all the output of the dependency's
                         build command was placed. This is useful for picking up
                         things like header files and such from other packages.

# A complete example

The code blocks below lay out a cargo project which has a small and simple C
dependency along with the necessary infrastructure for linking that to the rust
program.

```toml
# Cargo.toml
[package]

name = "hello-world-from-c"
version = "0.1.0"
authors = [ "wycats@gmail.com" ]
build = "make -C build"
```

```make
# build/Makefile

# Support cross compilation to/from 32/64 bit.
ARCH := $(word 1, $(subst -, ,$(TARGET)))
ifeq ($(ARCH),i686)
CFLAGS += -m32 -fPIC
else
CFLAGS += -m64 -fPIC
endif

all:
    $(CC) $(CFLAGS) hello.c -c -o $(OUT_DIR)/hello.o
    $(AR) crus $(OUT_DIR)/libhello.a $(OUT_DIR)/hello.o

```

```c
// build/hello.c
int foo() { return 1; }
```

```rust
// src/main.rs

#[link(name = "hello", kind = "static")]
extern {
    fn foo() -> i32;
}

fn main() {
    let number = unsafe { foo() };
    println!("found {} from C!", number);
}
```
