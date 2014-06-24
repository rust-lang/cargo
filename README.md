Cargo downloads your Rust project’s dependencies and compiles your project.

Learn more at http://crates.io/.

## Compiling cargo

You'll want to clone cargo using --recursive on git, to clone in it's submodule
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

## License

Cargo is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See LICENSE-APACHE and LICENSE-MIT for details.
