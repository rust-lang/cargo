% Building external code

Some packages need to compile third-party non-Rust code, for example C
libraries.

Cargo does not aim to replace other tools that are well-optimized for
building C or C++ code, but it does integrate with them with the `build`
configuration option.

```toml
[package]

name = "hello-world-from-c"
version = "0.0.1"
authors = [ "you@example.com" ]
links = ["myclib"]
build = "build.rs"
```

The Rust file designated by the `build` command will be compiled and invoked
before anything else is compiled in the package, allowing your Rust code to
depend on the built artifacts.

If the `links` entry is present, it is the responsibility of this build script
to indicate Cargo how to link to each specified library.

Here's what you need to know:

* Cargo passes the list of C libraries specified by `links` that must be
  built as arguments to the script. Using a `.cargo/config` file, the user can
  choose to use prebuilt libraries instead, in which case these prebuilt
  libraries will *not* be passed to the build script.
* Your build script should pass informations back to Cargo by writing to
  stdout. Writing `cargo:rustc-flags=-L /path -l foo` will
  add the `-L /path` and `-l foo` flags to rustc whenever it is invoked.
* You can use Rust libraries within your build script by adding a
  `[build-dependencies]` section in the manifest similar to `[dependencies]`.
* Build scripts don't need to actually *build* anything, you can simply
  return the location of an existing library in the filesystem if you wish so.
  This is the recommended way to do for libraries that are available in the
  platform's dependencies manager.
* Cargo passes your build script an environment variable named
  `OUT_DIR`, which is where you should put any compiled artifacts.
* Cargo will retain all output in `OUT_DIR` for clean packages across
  builds (intelligently discarding the compiled artifacts for dirty
  dependencies). Do not put the output of a build command in any other
  directory.
* The actual location of `$OUT_DIR` is
  `/path/to/project/target/native/$your-out-dir`.
* The target triple that the build command should compile for is specified by
  the `TARGET` environment variable.

# Environment Variables

The following environment variables are always available for build
commands.

* `OUT_DIR` - the folder in which all output should be placed.
* `TARGET` - the target triple that is being compiled for. Native code should be
             compiled for this triple.
* `NUM_JOBS` - the parallelism specified as the top-level parallelism. This can
               be useful to pass a `-j` parameter to a system like `make`.
* `DEP_<name>_<key>` - This variable is present for all immediate dependencies
                       of the package being built. The `<name>` will be the
                       package's name, in uppercase, with `-` characters
                       translated to a `_`. The `<key>` is a user-defined key
                       written by the dependency's build script.
* `CARGO_MANIFEST_DIR` - The directory containing the manifest for the package
                         being built. Note that this is the package Cargo is
                         being run on, not the package of the build script.
* `OPT_LEVEL`, `DEBUG` - values of the corresponding variables for the
                         profile currently being built.
* `PROFILE` - name of the profile currently being built (see
              [profiles][profile]).
* `CARGO_FEATURE_<name>` - For each activated feature of the package being
                           built, this environment variable will be present
                           where `<name>` is the name of the feature uppercased
                           and having `-` translated to `_`.

In addition to this, the `OUT_DIR` variable will also be available when
a regular library or binary of this package is compiled, thus giving you
access to the generated files thanks to the `include!`, `include_str!`
or `include_bin!` macros.

[profile]: manifest.html#the-[profile.*]-sections

# Metadata

All the lines printed to stdout by a build script that start with `cargo:`
are interpreted by Cargo and must be of the form `key=value`.

Example output:

```
cargo:rustc-flags=-l static:foo -L /path/to/foo
cargo:root=/path/to/foo
cargo:libdir=/path/to/foo/lib
cargo:include=/path/to/foo/include
```

The `rustc-flags` key is special and indicates the flags that Cargo will
pass to Rustc.

Any other element is a user-defined metadata that will be passed via
the `DEP_<name>_<key>` environment variables to packages that immediatly
depend on the package containing the build script.

# A complete example: C dependency

The code blocks below lay out a cargo project which has a small and simple C
dependency along with the necessary infrastructure for linking that to the rust
program.

```toml
# Cargo.toml
[package]

name = "hello-world-from-c"
version = "0.0.1"
authors = [ "you@example.com" ]
build = "build.rs"
```

```rust
// build.rs
use std::io::Command;

fn main() {
    let out_dir = std::os::getenv("OUT_DIR");

    // note: this code is deliberately naive
    // it is highly recommended that you use a library dedicated to building C code
    // instead of manually calling gcc

    Command::new("gcc").arg("build/hello.c")
                       .arg("-shared")
                       .arg("-o")
                       .arg(format!("{}/libhello.so", out_dir))
                       .output()
                       .unwrap();

    println!("cargo:rustc-flags=-L {} -l hello", out_dir);
}
```

```c
// build/hello.c
int foo() { return 1; }
```

```rust
// src/main.rs
extern crate libc;

extern {
    fn foo() -> libc::c_int;
}

fn main() {
    let number = unsafe { foo() };
    println!("found {} from C!", number);
}
```

# A complete example: generating Rust code

The follow code generates a Rust file in `${OUT_DIR}/generated.rs`
then processes its content in the *real* binary.

```toml
# Cargo.toml
[package]

name = "hello-world"
version = "0.0.1"
authors = [ "you@example.com" ]
build = "build.rs"
```

```rust
// build.rs
use std::io::fs::File;

fn main() {
    let out_dir = std::os::getenv("OUT_DIR");
    let path = Path::new(out_dir).join("generated.rs");

    let mut file = File::create(&path).unwrap();
    (write!(file, r#"fn say_hello() { \
                        println!("hello world"); \
                     }"#)).unwrap();
}
```

```rust
// src/main.rs

include!(concat!(env!("OUT_DIR"), "/generated.rs"))

fn main() {
    say_hello();
}
```