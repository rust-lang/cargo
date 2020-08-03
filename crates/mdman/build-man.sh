#!/bin/bash

set -e

cargo run -- -t md -o doc/out doc/*.md
cargo run -- -t txt -o doc/out doc/*.md
cargo run -- -t man -o doc/out doc/*.md
