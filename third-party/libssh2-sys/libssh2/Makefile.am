# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
AUTOMAKE_OPTIONS = foreign nostdinc

SUBDIRS = src docs
SUBDIRS += tests
if BUILD_EXAMPLES
SUBDIRS += example
endif

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = libssh2.pc

include_HEADERS = \
  include/libssh2.h \
  include/libssh2_publickey.h \
  include/libssh2_sftp.h

DISTCLEANFILES =

VMSFILES = vms/libssh2_make_example.dcl vms/libssh2_make_help.dcl \
  vms/libssh2_make_kit.dcl vms/libssh2_make_lib.dcl vms/man2help.c \
  vms/readme.vms vms/libssh2_config.h

WIN32FILES = src/libssh2.rc

OS400FILES = os400/README400 os400/initscript.sh os400/make.sh \
  os400/make-src.sh os400/make-rpg.sh os400/make-include.sh \
  os400/config400.default \
  os400/os400sys.c os400/ccsid.c \
  os400/libssh2_config.h os400/macros.h os400/libssh2_ccsid.h \
  os400/include/alloca.h os400/include/sys/socket.h \
  os400/include/assert.h \
  os400/libssh2rpg/libssh2.rpgle.in \
  os400/libssh2rpg/libssh2_ccsid.rpgle.in \
  os400/libssh2rpg/libssh2_publickey.rpgle \
  os400/libssh2rpg/libssh2_sftp.rpgle \
  os400/rpg-examples/SFTPXMPLE

EXTRA_DIST = $(WIN32FILES) get_ver.awk \
  maketgz RELEASE-NOTES libssh2.pc.in $(VMSFILES) config.rpath \
  CMakeLists.txt cmake git2news.pl libssh2-style.el README.md $(OS400FILES)

ACLOCAL_AMFLAGS = -I m4

.PHONY: ChangeLog
ChangeLog:
	echo "see NEWS" > ./ChangeLog
DISTCLEANFILES += ChangeLog

dist-hook:
	rm -rf $(top_builddir)/tests/log
	find $(distdir) -name "*.dist" -exec rm {} \;
	(distit=`find $(srcdir) -name "*.dist"`; \
	for file in $$distit; do \
	  strip=`echo $$file | sed -e s/^$(srcdir)// -e s/\.dist//`; \
	  cp -p $$file $(distdir)$$strip; \
	done)

# Code Coverage

init-coverage:
	make clean
	lcov --directory . --zerocounters

COVERAGE_CCOPTS := "-g --coverage"
COVERAGE_OUT := docs/coverage

build-coverage:
	make CFLAGS=$(COVERAGE_CCOPTS) check
	mkdir -p $(COVERAGE_OUT)
	lcov --directory . --output-file $(COVERAGE_OUT)/$(PACKAGE).info \
	  --capture

gen-coverage:
	genhtml --output-directory $(COVERAGE_OUT) \
	  $(COVERAGE_OUT)/$(PACKAGE).info \
	  --highlight --frames --legend \
	  --title "$(PACKAGE_NAME)"

coverage: init-coverage build-coverage gen-coverage

checksrc:
	ci/checksrc.sh
