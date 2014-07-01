RUSTC_FLAGS ?=
DESTDIR ?=
PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin

ifeq ($(wildcard rustc/bin),)
export RUSTC := rustc
else
export RUSTC := $(CURDIR)/rustc/bin/rustc
endif

export PATH := $(PATH):$(CURDIR)/rustc/bin

# Link flags to pull in dependencies
BINS = cargo \
	     cargo-build \
	     cargo-read-manifest \
	     cargo-rustc \
	     cargo-verify-project \
	     cargo-git-checkout \
		 cargo-test \

SRC = $(shell find src -name '*.rs' -not -path 'src/bin*')

DEPS = -L libs/hammer.rs/target -L libs/toml-rs/build
TOML = libs/toml-rs/build/$(shell $(RUSTC) --crate-file-name libs/toml-rs/src/toml.rs)
HAMMER = libs/hammer.rs/target/$(shell $(RUSTC) --crate-type=lib --crate-file-name libs/hammer.rs/src/hammer.rs)
HAMCREST = libs/hamcrest-rust/target/libhamcrest.timestamp
LIBCARGO = target/libcargo.timestamp
BIN_TARGETS = $(patsubst %,target/%,$(BINS))

all: $(BIN_TARGETS)

# === Dependencies

$(HAMMER): $(wildcard libs/hammer.rs/src/*.rs)
	$(MAKE) -C libs/hammer.rs

$(TOML): $(wildcard libs/toml-rs/src/*.rs)
	$(MAKE) -C libs/toml-rs

$(HAMCREST): $(shell find libs/hamcrest-rust/src/hamcrest -name '*.rs')
	$(MAKE) -C libs/hamcrest-rust

# === Cargo

$(LIBCARGO): $(SRC) $(HAMMER) $(TOML)
	mkdir -p target
	$(RUSTC) $(RUSTC_FLAGS) $(DEPS) --out-dir target src/cargo/lib.rs
	touch $(LIBCARGO)

libcargo: $(LIBCARGO)

# === Commands

$(BIN_TARGETS): target/%: src/bin/%.rs $(HAMMER) $(TOML) $(LIBCARGO)
	$(RUSTC) $(RUSTC_FLAGS) $(DEPS) -Ltarget --out-dir target $<

# === Tests

TEST_SRC = $(shell find tests -name '*.rs')
TEST_DEPS = $(DEPS) -L libs/hamcrest-rust/target

target/tests/test-integration: $(HAMCREST) $(TEST_SRC) $(BIN_TARGETS)
	$(RUSTC) --test --crate-type=lib $(TEST_DEPS) -Ltarget -o $@  tests/tests.rs

target/tests/test-unit: $(TOML) $(HAMCREST) $(SRC) $(HAMMER)
	mkdir -p target/tests
	$(RUSTC) --test $(RUSTC_FLAGS) $(TEST_DEPS) -o $@ src/cargo/lib.rs

test-unit: target/tests/test-unit
	target/tests/test-unit $(only)

test-integration: target/tests/test-integration
	$< $(only)

test: test-unit test-integration style

style:
	sh tests/check-style.sh

clean:
	rm -rf target

clean-all: clean
	git submodule foreach make clean

install:
	install -d $(DESTDIR)$(BINDIR)
	install target/cargo target/cargo-* $(DESTDIR)$(BINDIR)

# Setup phony tasks
.PHONY: all clean distclean test test-unit test-integration libcargo style

# Disable unnecessary built-in rules
.SUFFIXES:


