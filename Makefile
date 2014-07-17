RUSTC_FLAGS ?=
DESTDIR ?=
PREFIX ?= /usr/local
TARGET ?= target
PKG_NAME ?= cargo-nightly

ifeq ($(wildcard rustc/bin),)
export RUSTC := rustc
else
export RUSTC := $(CURDIR)/rustc/bin/rustc
export LD_LIBRARY_PATH := $(CURDIR)/rustc/lib:$(LD_LIBRARY_PATH)
export DYLD_LIBRARY_PATH := $(CURDIR)/rustc/lib:$(DYLD_LIBRARY_PATH)
endif

CFG_RELEASE=0.1.0-pre
CFG_VER_DATE = $(shell git log -1 --pretty=format:'%ai')
CFG_VER_HASH = $(shell git rev-parse --short HEAD)
CFG_VERSION = $(PKG_NAME) $(CFG_RELEASE) ($(CFG_VER_HASH) $(CFG_VER_DATE))

export CFG_RELEASE
export CFG_VER_DATE
export CFG_VER_HASH
export CFG_VERSION

export PATH := $(CURDIR)/rustc/bin:$(PATH)

# Link flags to pull in dependencies
BINS = cargo \
	     cargo-build \
	     cargo-clean \
	     cargo-read-manifest \
	     cargo-rustc \
	     cargo-verify-project \
	     cargo-git-checkout \
		 cargo-test \
		 cargo-run \
		 cargo-version

SRC = $(shell find src -name '*.rs' -not -path 'src/bin*')

ifeq ($(OS),Windows_NT)
X = .exe
endif

DEPS = -L libs/hammer.rs/target -L libs/toml-rs/build
TOML = libs/toml-rs/build/$(shell $(RUSTC) --print-file-name libs/toml-rs/src/toml.rs)
HAMMER = libs/hammer.rs/target/$(shell $(RUSTC) --crate-type=lib --print-file-name libs/hammer.rs/src/hammer.rs)
HAMCREST = libs/hamcrest-rust/target/libhamcrest.timestamp
LIBCARGO = $(TARGET)/libcargo.rlib
TESTDIR = $(TARGET)/tests
DISTDIR = $(TARGET)/dist
PKGDIR = $(DISTDIR)/$(PKG_NAME)
BIN_TARGETS = $(BINS:%=$(TARGET)/%$(X))

all: $(BIN_TARGETS)

# === Dependencies

$(HAMMER): $(wildcard libs/hammer.rs/src/*.rs)
	$(MAKE) -C libs/hammer.rs

$(TOML): $(wildcard libs/toml-rs/src/*.rs)
	$(MAKE) -C libs/toml-rs

$(HAMCREST): $(shell find libs/hamcrest-rust/src/hamcrest -name '*.rs')
	$(MAKE) -C libs/hamcrest-rust

$(TARGET)/:
	mkdir -p $@

$(TESTDIR)/:
	mkdir -p $@

# === Cargo

$(LIBCARGO): $(SRC) $(HAMMER) $(TOML) | $(TARGET)/
	$(RUSTC) $(RUSTC_FLAGS) $(DEPS) --out-dir $(TARGET) src/cargo/lib.rs

libcargo: $(LIBCARGO)

# === Commands

$(BIN_TARGETS): $(TARGET)/%$(X): src/bin/%.rs $(HAMMER) $(TOML) $(LIBCARGO)
	$(RUSTC) $(RUSTC_FLAGS) $(DEPS) -L$(TARGET) --out-dir $(TARGET) $<

# === Tests

TEST_SRC = $(shell find tests -name '*.rs')
TEST_DEPS = $(DEPS) -L libs/hamcrest-rust/target

$(TESTDIR)/test-integration: $(HAMCREST) $(TEST_SRC) $(BIN_TARGETS) | $(TESTDIR)/
	$(RUSTC) --test $(TEST_DEPS) -L$(TARGET) -o $@ tests/tests.rs

$(TESTDIR)/test-unit: $(TOML) $(HAMCREST) $(SRC) $(HAMMER) | $(TESTDIR)/
	$(RUSTC) --test -g $(RUSTC_FLAGS) $(TEST_DEPS) -o $@ src/cargo/lib.rs

test-unit: $(TESTDIR)/test-unit
	$< $(only)

test-integration: $(TESTDIR)/test-integration
	$< $(only)

test: test-unit test-integration style no-exes

style:
	sh tests/check-style.sh

no-exes:
	find $$(git ls-files | grep -v '^lib') -perm +111 -type f \
		-not -name '*.sh' -not -name '*.rs' | grep '.*' \
		&& exit 1 || exit 0

clean:
	rm -rf $(TARGET)

clean-all: clean
	git submodule foreach make clean

dist: $(DISTDIR)/$(PKG_NAME).tar.gz

distcheck: dist
	rm -rf $(TARGET)/distcheck
	mkdir -p $(TARGET)/distcheck
	(cd $(TARGET)/distcheck && tar xf ../dist/$(PKG_NAME).tar.gz)
	$(TARGET)/distcheck/$(PKG_NAME)/install.sh \
		--prefix=$(TARGET)/distcheck/install
	$(TARGET)/distcheck/install/bin/cargo -h > /dev/null
	$(TARGET)/distcheck/$(PKG_NAME)/install.sh \
		--prefix=$(TARGET)/distcheck/install --uninstall
	[ -f $(TARGET)/distcheck/install/bin/cargo ] && exit 1 || exit 0

$(DISTDIR)/$(PKG_NAME).tar.gz: $(PKGDIR)/lib/cargo/manifest.in
	tar -czvf $@ -C $(DISTDIR) $(PKG_NAME)

$(PKGDIR)/lib/cargo/manifest.in: $(BIN_TARGETS) Makefile
	rm -rf $(PKGDIR)
	mkdir -p $(PKGDIR)/bin $(PKGDIR)/lib/cargo
	cp $(TARGET)/cargo$(X) $(PKGDIR)/bin
	cp $(BIN_TARGETS) $(PKGDIR)/lib/cargo
	rm $(PKGDIR)/lib/cargo/cargo$(X)
	(cd $(PKGDIR) && find . -type f | sed 's/^\.\///') \
		> $(DISTDIR)/manifest-$(PKG_NAME).in
	cp src/install.sh $(PKGDIR)
	cp README.md LICENSE-MIT LICENSE-APACHE $(PKGDIR)
	cp LICENSE-MIT $(PKGDIR)
	mv $(DISTDIR)/manifest-$(PKG_NAME).in $(PKGDIR)/lib/cargo/manifest.in

install: $(PKGDIR)/lib/cargo/manifest.in
	$(PKGDIR)/install.sh --prefix=$(PREFIX) --destdir=$(DESTDIR)

# Setup phony tasks
.PHONY: all clean distclean test test-unit test-integration libcargo style

# Disable unnecessary built-in rules
.SUFFIXES:


