RUSTC_TARGET = target
RUSTC_FLAGS ?= --out-dir $(RUSTC_TARGET) -L $(RUSTC_TARGET)/libs

TOML_LIB := $(shell rustc --crate-file-name libs/rust-toml/src/toml/lib.rs)
HAMMER_LIB := $(shell rustc --crate-file-name libs/hammer.rs/src/lib.rs)
LIBCARGO_LIB := $(shell rustc --crate-file-name libcargo/cargo.rs)

default: dependencies commands

dependencies: target/libs/$(TOML_LIB) target/libs/$(HAMMER_LIB) target/libs/$(LIBCARGO_LIB)

commands: target/cargo-rustc target/cargo-verify-project target/cargo-read-manifest

clean:
	rm -rf target

target/cargo-rustc: target dependencies target/libs/$(TOML_LIB) commands/cargo-rustc/main.rs
	rustc commands/cargo-rustc/main.rs $(RUSTC_FLAGS)

target/cargo-verify-project: target dependencies target/libs/$(TOML_LIB) commands/cargo-verify-project/main.rs
	rustc commands/cargo-verify-project/main.rs $(RUSTC_FLAGS)

target/cargo-read-manifest: target dependencies target/libs/$(TOML_LIB) target/libs/$(HAMMER_LIB) commands/cargo-read-manifest/main.rs
	rustc commands/cargo-read-manifest/main.rs $(RUSTC_FLAGS)

target/libs/$(TOML_LIB): target libs/rust-toml/src/toml/lib.rs
	cd libs/rust-toml && make
	cp libs/rust-toml/lib/*.rlib target/libs

target/libs/$(HAMMER_LIB): target libs/hammer.rs/src/lib.rs
	cd libs/hammer.rs && make
	cp libs/hammer.rs/target/*.rlib target/libs

target/libs/$(LIBCARGO_LIB): target libcargo/cargo.rs
	cd libcargo && make
	cp libcargo/target/*.rlib target/libs/

target:
	mkdir -p $(RUSTC_TARGET)/libs

.PHONY: default clean dependencies commands
