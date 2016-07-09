# rustfix

> **HIGHLY EXPERIMENTAL – MIGHT EAT YOUR CODE**

The goal of this tool is to read and apply the suggestions made by rustc (and third-party lints, like those offered by [Clippy](https://github.com/Manishearth/rust-clippy)).

## Current state

This tool can

- read a file of diagnostics (one JSON object per line)
- interactively step through the suggestions and ask the user what to do
- apply suggestions (currently whole lines only)

![rustfix demo](http://i.imgur.com/E9YkK76.png)

## Get the example running

My current example output for diagnostics is based on [libui-rs](https://github.com/pcwalton/libui-rs). You can find the example JSON in `tests/fixtures/libui-rs/clippy.json`.

Run `rustfix`:

```sh
$ cd tests/fixtures/libui-rs/
$ cargo run -- clippy.json
```

### Generate the diagnostics JSON file yourself

```sh
$ git clone https://github.com/pcwalton/libui-rs.git
# HEAD is at 13299d28f69f8009be8e08e453a9b0024f153a60
$ cd libui-rs/ui/
$ cargo clippy -- -Z unstable-options --error-format json &2> clippy.json
# Manually remove the first line ("Compiling....")
```

## Gotchas

- rustc JSON output is unstable
- Not all suggestions can be applied trivially (e.g. clippy's "You should use `Default` instead of that `fn new()` you just wrote"-lint will replace your `new`-method with an `impl` block -- which is obvisouly invalid syntax.)
- This tool _will_ eat your laundry

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
