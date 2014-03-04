RUSTC_TARGET = target
RUSTC_FLAGS ?= --out-dir $(RUSTC_TARGET)

default: target/cargo-rustc

clean:
	rm -rf target

target/cargo-rustc: target commands/cargo-rustc/main.rs
	rustc commands/cargo-rustc/main.rs $(RUSTC_FLAGS)

target:
	mkdir -p $(RUSTC_TARGET)

.PHONY: default clean
