#!/bin/bash
#
# This script builds the Cargo man pages.
#
# The source for the man pages are located in src/doc/man/ in markdown format.
# These also are handlebars templates, see crates/mdman/README.md for details.
#
# The generated man pages are placed in the src/etc/man/ directory. The pages
# are also expanded into markdown (after being expanded by handlebars) and
# saved in the src/doc/src/commands/ directory. These are included in the
# Cargo book, which is converted to HTML by mdbook.

set -e

cd "$(dirname "${BASH_SOURCE[0]}")"

OPTIONS="--url https://doc.rust-lang.org/cargo/commands/ \
    --man rustc:1=https://doc.rust-lang.org/rustc/index.html \
    --man rustdoc:1=https://doc.rust-lang.org/rustdoc/index.html"

cargo run --manifest-path=../../crates/mdman/Cargo.toml -- \
    -t md -o src/commands man/cargo*.md \
    $OPTIONS

cargo run --manifest-path=../../crates/mdman/Cargo.toml -- \
    -t txt -o man/generated_txt man/cargo*.md \
    $OPTIONS

cargo run --manifest-path=../../crates/mdman/Cargo.toml -- \
    -t man -o ../etc/man man/cargo*.md \
    $OPTIONS
