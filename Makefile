RUSTC_TARGET = target
RUSTC_FLAGS ?= --out-dir $(RUSTC_TARGET) -L $(RUSTC_TARGET)/libs

TOML_LIB := $(shell rustc --crate-file-name libs/rust-toml/src/toml/lib.rs)

default: target/cargo-rustc target/cargo-verify-project

clean:
	rm -rf target

target/cargo-rustc: target target/libs/$(TOML_LIB) commands/cargo-rustc/main.rs
	rustc commands/cargo-rustc/main.rs $(RUSTC_FLAGS)

target/cargo-verify-project: target target/libs/$(TOML_LIB) commands/cargo-verify-project/main.rs
	rustc commands/cargo-verify-project/main.rs $(RUSTC_FLAGS)

target/libs/$(TOML_LIB): libs/rust-toml/src/toml/lib.rs
	cd libs/rust-toml && make
	cp libs/rust-toml/lib/*.rlib target/libs

target:
	mkdir -p $(RUSTC_TARGET)/libs

.PHONY: default clean
