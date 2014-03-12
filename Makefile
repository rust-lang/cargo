RUSTC ?= rustc
# RUSTC_FLAGS ?= --out-dir $(RUSTC_TARGET) -L $(RUSTC_TARGET)/libs

TOML_LIB := $(shell rustc --crate-file-name libs/rust-toml/src/toml/lib.rs)
HAMMER_LIB := $(shell rustc --crate-file-name libs/hammer.rs/src/hammer.rs)

# Link flags to pull in dependencies
DEPS = -L libs/hammer.rs/target -L libs/rust-toml/lib
SRC = $(wildcard src/*.rs)
BINS = cargo-read-manifest \
			 cargo-rustc \
			 cargo-verify-project

BIN_TARGETS = $(patsubst %,target/%,$(BINS))

all: $(BIN_TARGETS)

# Builds the hammer dependency
hammer:
	cd libs/hammer.rs && make

toml:
	cd libs/rust-toml && make

# === Cargo

target:
	mkdir -p target

libcargo: target $(SRC)
	$(RUSTC) --out-dir target src/cargo.rs

# === Commands

$(BIN_TARGETS): target/%: src/bin/%.rs hammer toml libcargo
	$(RUSTC) $(DEPS) -Ltarget --out-dir target $<

clean:
	rm -rf target

distclean: clean
	cd libs/hamcrest-rust && make clean
	cd libs/hammer.rs && make clean
	cd libs/rust-toml && make clean

.PHONY: all clean distclean test hammer libcargo
