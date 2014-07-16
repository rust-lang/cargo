DESTDIR ?=
PREFIX ?= /usr/local
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

ifeq ($(OS),Windows_NT)
X = .exe
endif

TARGET = target
DISTDIR = $(TARGET)/dist
PKGDIR = $(DISTDIR)/$(PKG_NAME)

BIN_TARGETS := $(wildcard src/bin/*.rs)
BIN_TARGETS := $(BIN_TARGETS:src/bin/%.rs=%)
BIN_TARGETS := $(filter-out cargo,$(BIN_TARGETS))
BIN_TARGETS := $(BIN_TARGETS:%=$(TARGET)/%$(X))

CARGO := $(TARGET)/snapshot/cargo-nightly/bin/cargo$(X)

all: $(CARGO)
	$(CARGO) build $(ARGS)

$(CARGO): src/snapshots.txt
	python src/etc/dl-snapshot.py
	touch $@


# === Tests

test: test-unit style no-exes

test-unit: $(CARGO)
	$(CARGO) test $(only)

style:
	sh tests/check-style.sh

no-exes:
	find $$(git ls-files) -perm +111 -type f \
		-not -name '*.sh' -not -name '*.rs' | grep '.*' \
		&& exit 1 || exit 0

# === Misc

clean-all: clean
clean:
	rm -rf $(TARGET)

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

$(PKGDIR)/lib/cargo/manifest.in: all
	rm -rf $(PKGDIR)
	mkdir -p $(PKGDIR)/bin $(PKGDIR)/lib/cargo
	cp $(TARGET)/cargo$(X) $(PKGDIR)/bin
	cp $(BIN_TARGETS) $(PKGDIR)/lib/cargo
	(cd $(PKGDIR) && find . -type f | sed 's/^\.\///') \
		> $(DISTDIR)/manifest-$(PKG_NAME).in
	cp src/etc/install.sh $(PKGDIR)
	cp README.md LICENSE-MIT LICENSE-APACHE $(PKGDIR)
	cp LICENSE-MIT $(PKGDIR)
	mv $(DISTDIR)/manifest-$(PKG_NAME).in $(PKGDIR)/lib/cargo/manifest.in

install: $(PKGDIR)/lib/cargo/manifest.in
	$(PKGDIR)/install.sh --prefix=$(PREFIX) --destdir=$(DESTDIR)

# Setup phony tasks
.PHONY: all clean test test-unit style

# Disable unnecessary built-in rules
.SUFFIXES:


