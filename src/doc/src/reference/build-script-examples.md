# Build Script Examples

The following sections illustrate some examples of writing build scripts.

Some common build script functionality can be found via crates on [crates.io].
Check out the [`build-dependencies`
keyword](https://crates.io/keywords/build-dependencies) to see what is
available. The following is a sample of some popular crates[^†]:

* [`bindgen`](https://crates.io/crates/bindgen) --- Automatically generate Rust
  FFI bindings to C libraries.
* [`cc`](https://crates.io/crates/cc) --- Compiles C/C++/assembly.
* [`pkg-config`](https://crates.io/crates/pkg-config) --- Detect system
  libraries using the `pkg-config` utility.
* [`cmake`](https://crates.io/crates/cmake) --- Runs the `cmake` build tool to build a native library.
* [`autocfg`](https://crates.io/crates/autocfg),
  [`rustc_version`](https://crates.io/crates/rustc_version),
  [`version_check`](https://crates.io/crates/version_check) --- These crates
  provide ways to implement conditional compilation based on the current
  `rustc` such as the version of the compiler.

[^†]: This list is not an endorsement. Evaluate your dependencies to see which
is right for your project.

## Code generation

Some Cargo packages need to have code generated just before they are compiled
for various reasons. Here we’ll walk through a simple example which generates a
library call as part of the build script.

First, let’s take a look at the directory structure of this package:

```text
.
├── Cargo.toml
├── build.rs
└── src
    └── main.rs

1 directory, 3 files
```

Here we can see that we have a `build.rs` build script and our binary in
`main.rs`. This package has a basic manifest:

```toml
# Cargo.toml

[package]
name = "hello-from-generated-code"
version = "0.1.0"
edition = "2021"
```

Let’s see what’s inside the build script:

```rust,no_run
// build.rs

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("hello.rs");
    fs::write(
        &dest_path,
        "pub fn message() -> &'static str {
            \"Hello, World!\"
        }
        "
    ).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
```

There’s a couple of points of note here:

* The script uses the `OUT_DIR` environment variable to discover where the
  output files should be located. It can use the process’ current working
  directory to find where the input files should be located, but in this case we
  don’t have any input files.
* In general, build scripts should not modify any files outside of `OUT_DIR`.
  It may seem fine on the first blush, but it does cause problems when you use
  such crate as a dependency, because there's an *implicit* invariant that
  sources in `.cargo/registry` should be immutable. `cargo` won't allow such
  scripts when packaging.
* This script is relatively simple as it just writes out a small generated file.
  One could imagine that other more fanciful operations could take place such as
  generating a Rust module from a C header file or another language definition,
  for example.
* The [`rerun-if-changed` instruction](build-scripts.md#rerun-if-changed)
  tells Cargo that the build script only needs to re-run if the build script
  itself changes. Without this line, Cargo will automatically run the build
  script if any file in the package changes. If your code generation uses some
  input files, this is where you would print a list of each of those files.

Next, let’s peek at the library itself:

```rust,ignore
// src/main.rs

include!(concat!(env!("OUT_DIR"), "/hello.rs"));

fn main() {
    println!("{}", message());
}
```

This is where the real magic happens. The library is using the rustc-defined
[`include!` macro][include-macro] in combination with the
[`concat!`][concat-macro] and [`env!`][env-macro] macros to include the
generated file (`hello.rs`) into the crate’s compilation.

Using the structure shown here, crates can include any number of generated files
from the build script itself.

[include-macro]: ../../std/macro.include.html
[concat-macro]: ../../std/macro.concat.html
[env-macro]: ../../std/macro.env.html

## Building a native library

Sometimes it’s necessary to build some native C or C++ code as part of a
package. This is another excellent use case of leveraging the build script to
build a native library before the Rust crate itself. As an example, we’ll create
a Rust library which calls into C to print “Hello, World!”.

Like above, let’s first take a look at the package layout:

```text
.
├── Cargo.toml
├── build.rs
└── src
    ├── hello.c
    └── main.rs

1 directory, 4 files
```

Pretty similar to before! Next, the manifest:

```toml
# Cargo.toml

[package]
name = "hello-world-from-c"
version = "0.1.0"
edition = "2021"
```

For now we’re not going to use any build dependencies, so let’s take a look at
the build script now:

```rust,no_run
// build.rs

use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Note that there are a number of downsides to this approach, the comments
    // below detail how to improve the portability of these commands.
    Command::new("gcc").args(&["src/hello.c", "-c", "-fPIC", "-o"])
                       .arg(&format!("{}/hello.o", out_dir))
                       .status().unwrap();
    Command::new("ar").args(&["crus", "libhello.a", "hello.o"])
                      .current_dir(&Path::new(&out_dir))
                      .status().unwrap();

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=hello");
    println!("cargo:rerun-if-changed=src/hello.c");
}
```

This build script starts out by compiling our C file into an object file (by
invoking `gcc`) and then converting this object file into a static library (by
invoking `ar`). The final step is feedback to Cargo itself to say that our
output was in `out_dir` and the compiler should link the crate to `libhello.a`
statically via the `-l static=hello` flag.

Note that there are a number of drawbacks to this hard-coded approach:

* The `gcc` command itself is not portable across platforms. For example it’s
  unlikely that Windows platforms have `gcc`, and not even all Unix platforms
  may have `gcc`. The `ar` command is also in a similar situation.
* These commands do not take cross-compilation into account. If we’re cross
  compiling for a platform such as Android it’s unlikely that `gcc` will produce
  an ARM executable.

Not to fear, though, this is where a `build-dependencies` entry would help!
The Cargo ecosystem has a number of packages to make this sort of task much
easier, portable, and standardized. Let's try the [`cc`
crate](https://crates.io/crates/cc) from [crates.io]. First, add it to the
`build-dependencies` in `Cargo.toml`:

```toml
[build-dependencies]
cc = "1.0"
```

And rewrite the build script to use this crate:

```rust,ignore
// build.rs

fn main() {
    cc::Build::new()
        .file("src/hello.c")
        .compile("hello");
    println!("cargo:rerun-if-changed=src/hello.c");
}
```

The [`cc` crate] abstracts a range of build script requirements for C code:

* It invokes the appropriate compiler (MSVC for windows, `gcc` for MinGW, `cc`
  for Unix platforms, etc.).
* It takes the `TARGET` variable into account by passing appropriate flags to
  the compiler being used.
* Other environment variables, such as `OPT_LEVEL`, `DEBUG`, etc., are all
  handled automatically.
* The stdout output and `OUT_DIR` locations are also handled by the `cc`
  library.

Here we can start to see some of the major benefits of farming as much
functionality as possible out to common build dependencies rather than
duplicating logic across all build scripts!

Back to the case study though, let’s take a quick look at the contents of the
`src` directory:

```c
// src/hello.c

#include <stdio.h>

void hello() {
    printf("Hello, World!\n");
}
```

```rust,ignore
// src/main.rs

// Note the lack of the `#[link]` attribute. We’re delegating the responsibility
// of selecting what to link over to the build script rather than hard-coding
// it in the source file.
extern { fn hello(); }

fn main() {
    unsafe { hello(); }
}
```

And there we go! This should complete our example of building some C code from a
Cargo package using the build script itself. This also shows why using a build
dependency can be crucial in many situations and even much more concise!

We’ve also seen a brief example of how a build script can use a crate as a
dependency purely for the build process and not for the crate itself at runtime.

[`cc` crate]: https://crates.io/crates/cc

## Linking to system libraries

This example demonstrates how to link a system library and how the build
script is used to support this use case.

Quite frequently a Rust crate wants to link to a native library provided on
the system to bind its functionality or just use it as part of an
implementation detail. This is quite a nuanced problem when it comes to
performing this in a platform-agnostic fashion. It is best, if possible, to
farm out as much of this as possible to make this as easy as possible for
consumers.

For this example, we will be creating a binding to the system's zlib library.
This is a library that is commonly found on most Unix-like systems that
provides data compression. This is already wrapped up in the [`libz-sys`
crate], but for this example, we'll do an extremely simplified version. Check
out [the source code][libz-source] for the full example.

To make it easy to find the location of the library, we will use the
[`pkg-config` crate]. This crate uses the system's `pkg-config` utility to
discover information about a library. It will automatically tell Cargo what is
needed to link the library. This will likely only work on Unix-like systems
with `pkg-config` installed. Let's start by setting up the manifest:

```toml
# Cargo.toml

[package]
name = "libz-sys"
version = "0.1.0"
edition = "2021"
links = "z"

[build-dependencies]
pkg-config = "0.3.16"
```

Take note that we included the `links` key in the `package` table. This tells
Cargo that we are linking to the `libz` library. See ["Using another sys
crate"](#using-another-sys-crate) for an example that will leverage this.

The build script is fairly simple:

```rust,ignore
// build.rs

fn main() {
    pkg_config::Config::new().probe("zlib").unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
```

Let's round out the example with a basic FFI binding:

```rust,ignore
// src/lib.rs

use std::os::raw::{c_uint, c_ulong};

extern "C" {
    pub fn crc32(crc: c_ulong, buf: *const u8, len: c_uint) -> c_ulong;
}

#[test]
fn test_crc32() {
    let s = "hello";
    unsafe {
        assert_eq!(crc32(0, s.as_ptr(), s.len() as c_uint), 0x3610a686);
    }
}
```

Run `cargo build -vv` to see the output from the build script. On a system
with `libz` already installed, it may look something like this:

```text
[libz-sys 0.1.0] cargo:rustc-link-search=native=/usr/lib
[libz-sys 0.1.0] cargo:rustc-link-lib=z
[libz-sys 0.1.0] cargo:rerun-if-changed=build.rs
```

Nice! `pkg-config` did all the work of finding the library and telling Cargo
where it is.

It is not unusual for packages to include the source for the library, and
build it statically if it is not found on the system, or if a feature or
environment variable is set. For example, the real [`libz-sys` crate] checks the
environment variable `LIBZ_SYS_STATIC` or the `static` feature to build it
from source instead of using the system library. Check out [the
source][libz-source] for a more complete example.

[`libz-sys` crate]: https://crates.io/crates/libz-sys
[`pkg-config` crate]: https://crates.io/crates/pkg-config
[libz-source]: https://github.com/rust-lang/libz-sys

## Using another `sys` crate

When using the `links` key, crates may set metadata that can be read by other
crates that depend on it. This provides a mechanism to communicate information
between crates. In this example, we'll be creating a C library that makes use
of zlib from the real [`libz-sys` crate].

If you have a C library that depends on zlib, you can leverage the [`libz-sys`
crate] to automatically find it or build it. This is great for cross-platform
support, such as Windows where zlib is not usually installed. `libz-sys` [sets
the `include`
metadata](https://github.com/rust-lang/libz-sys/blob/3c594e677c79584500da673f918c4d2101ac97a1/build.rs#L156)
to tell other packages where to find the header files for zlib. Our build
script can read that metadata with the `DEP_Z_INCLUDE` environment variable.
Here's an example:

```toml
# Cargo.toml

[package]
name = "zuser"
version = "0.1.0"
edition = "2021"

[dependencies]
libz-sys = "1.0.25"

[build-dependencies]
cc = "1.0.46"
```

Here we have included `libz-sys` which will ensure that there is only one
`libz` used in the final library, and give us access to it from our build
script:

```rust,ignore
// build.rs

fn main() {
    let mut cfg = cc::Build::new();
    cfg.file("src/zuser.c");
    if let Some(include) = std::env::var_os("DEP_Z_INCLUDE") {
        cfg.include(include);
    }
    cfg.compile("zuser");
    println!("cargo:rerun-if-changed=src/zuser.c");
}
```

With `libz-sys` doing all the heavy lifting, the C source code may now include
the zlib header, and it should find the header, even on systems where it isn't
already installed.

```c
// src/zuser.c

#include "zlib.h"

// … rest of code that makes use of zlib.
```

## Conditional compilation

A build script may emit [`rustc-cfg` instructions] which can enable conditions
that can be checked at compile time. In this example, we'll take a look at how
the [`openssl` crate] uses this to support multiple versions of the OpenSSL
library.

The [`openssl-sys` crate] implements building and linking the OpenSSL library.
It supports multiple different implementations (like LibreSSL) and multiple
versions. It makes use of the `links` key so that it may pass information to
other build scripts. One of the things it passes is the `version_number` key,
which is the version of OpenSSL that was detected. The code in the build
script looks something [like
this](https://github.com/sfackler/rust-openssl/blob/dc72a8e2c429e46c275e528b61a733a66e7877fc/openssl-sys/build/main.rs#L216):

```rust,ignore
println!("cargo:version_number={:x}", openssl_version);
```

This instruction causes the `DEP_OPENSSL_VERSION_NUMBER` environment variable
to be set in any crates that directly depend on `openssl-sys`.

The `openssl` crate, which provides the higher-level interface, specifies
`openssl-sys` as a dependency. The `openssl` build script can read the
version information generated by the `openssl-sys` build script with the
`DEP_OPENSSL_VERSION_NUMBER` environment variable. It uses this to generate
some [`cfg`
values](https://github.com/sfackler/rust-openssl/blob/dc72a8e2c429e46c275e528b61a733a66e7877fc/openssl/build.rs#L18-L36):

```rust,ignore
// (portion of build.rs)

if let Ok(version) = env::var("DEP_OPENSSL_VERSION_NUMBER") {
    let version = u64::from_str_radix(&version, 16).unwrap();

    if version >= 0x1_00_01_00_0 {
        println!("cargo:rustc-cfg=ossl101");
    }
    if version >= 0x1_00_02_00_0 {
        println!("cargo:rustc-cfg=ossl102");
    }
    if version >= 0x1_01_00_00_0 {
        println!("cargo:rustc-cfg=ossl110");
    }
    if version >= 0x1_01_00_07_0 {
        println!("cargo:rustc-cfg=ossl110g");
    }
    if version >= 0x1_01_01_00_0 {
        println!("cargo:rustc-cfg=ossl111");
    }
}
```

These `cfg` values can then be used with the [`cfg` attribute] or the [`cfg`
macro] to conditionally include code. For example, SHA3 support was added in
OpenSSL 1.1.1, so it is [conditionally
excluded](https://github.com/sfackler/rust-openssl/blob/dc72a8e2c429e46c275e528b61a733a66e7877fc/openssl/src/hash.rs#L67-L85)
for older versions:

```rust,ignore
// (portion of openssl crate)

#[cfg(ossl111)]
pub fn sha3_224() -> MessageDigest {
    unsafe { MessageDigest(ffi::EVP_sha3_224()) }
}
```

Of course, one should be careful when using this, since it makes the resulting
binary even more dependent on the build environment. In this example, if the
binary is distributed to another system, it may not have the exact same shared
libraries, which could cause problems.

[`cfg` attribute]: ../../reference/conditional-compilation.md#the-cfg-attribute
[`cfg` macro]: ../../std/macro.cfg.html
[`rustc-cfg` instructions]: build-scripts.md#rustc-cfg
[`openssl` crate]: https://crates.io/crates/openssl
[`openssl-sys` crate]: https://crates.io/crates/openssl-sys

[crates.io]: https://crates.io/
