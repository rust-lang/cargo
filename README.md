Cargo downloads your Rust project’s dependencies and compiles your project.

Learn more at http://crates.io/.

## Installing cargo

Cargo has nightlies available for use. The cargo source is not always guaranteed
to compile on rust master as it may lag behind by a day or two. Nightlies,
however, will run regardless of this fact!

```
$ curl -O http://static.rust-lang.org/cargo-dist/cargo-nightly-linux.tar.gz
$ tar xf cargo-nightly-linux.tar.gz
$ ./cargo-nightly/bin/cargo build
```

The current nightlies available are:

* `cargo-nightly-linux`
* `cargo-nightly-win`
* `cargo-nightly-mac`

## Compiling cargo

You'll want to clone cargo using --recursive on git, to clone in its submodule
dependencies.
```
$ git clone --recursive https://github.com/rust-lang/cargo
```
or
```
$ git submodule init
$ git submodule update
```
Then it's as simple as ```make``` followed by ```make install``` and you're
ready to go.

## Contributing to the Docs

To contribute to the docs, please submit pull requests to [wycats/cargo-website][1].
All you need to do is change the markdown files in the source directory.

[1]: https://github.com/wycats/cargo-website

## License

Cargo is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See LICENSE-APACHE and LICENSE-MIT for details.
