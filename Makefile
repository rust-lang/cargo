RUSTC ?= rustc
RUSTC_FLAGS ?=

# Link flags to pull in dependencies
BINS = cargo-compile \
	   cargo-read-manifest \
	   cargo-rustc \
	   cargo-verify-project

SRC = $(shell find src -name '*.rs')

DEPS = -L libs/hammer.rs/target -L libs/rust-toml/lib
TOML = libs/rust-toml/lib/$(shell rustc --crate-file-name libs/rust-toml/src/toml/lib.rs)
HAMMER = libs/hammer.rs/target/$(shell rustc --crate-type=lib --crate-file-name libs/hammer.rs/src/hammer.rs)
HAMCREST = libs/hamcrest-rust/target/timestamp
LIBCARGO = target/libcargo.timestamp
BIN_TARGETS = $(patsubst %,target/%,$(BINS))

all: $(BIN_TARGETS)

# === Dependencies

$(HAMMER): $(wildcard libs/hammer.rs/src/*.rs)
	cd libs/hammer.rs && make

$(TOML): $(wildcard libs/rust-toml/src/toml/*.rs)
	cd libs/rust-toml && make

$(HAMCREST): $(wildcard libs/hamcrest-rust/src/*.rs)
	cd libs/hamcrest-rust && make

# === Cargo

$(LIBCARGO): $(SRC)
	mkdir -p target
	$(RUSTC) $(RUSTC_FLAGS) --out-dir target src/cargo/mod.rs
	touch $(LIBCARGO)

libcargo: $(LIBCARGO)

# === Commands

$(BIN_TARGETS): target/%: src/bin/%.rs $(HAMMER) $(TOML) $(LIBCARGO)
	$(RUSTC) $(RUSTC_FLAGS) $(DEPS) -Ltarget --out-dir target $<

# === Tests

TEST_SRC = $(wildcard tests/*.rs)
TEST_DEPS = $(DEPS) -L libs/hamcrest-rust/target

tests/tests: $(BIN_TARGETS) $(HAMCREST) $(TEST_SRC)
	$(RUSTC) --test --crate-type=lib $(TEST_DEPS) -Ltarget --out-dir tests tests/tests.rs

test-integration: tests/tests
	CARGO_BIN_PATH=$(PWD)/target/ tests/tests

test: test-integration

clean:
	rm -rf target
	rm -f tests/tests

distclean: clean
	cd libs/hamcrest-rust && make clean
	cd libs/hammer.rs && make clean
	cd libs/rust-toml && make clean

# Setup phony tasks
.PHONY: all clean distclean test test-integration libcargo

# Disable unnecessary built-in rules
.SUFFIXES:

