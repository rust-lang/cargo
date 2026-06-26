# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
SUBDIRS = ossfuzz

AM_CPPFLAGS = -I$(top_builddir)/src -I$(top_srcdir)/src -I$(top_srcdir)/include

# This might hold -Werror
AM_CFLAGS = @LIBSSH2_CFLAG_EXTRAS@

# Get DOCKER_TESTS, DOCKER_TESTS_STATIC, STANDALONE_TESTS, STANDALONE_TESTS_STATOC, SSHD_TESTS,
# librunner_la_SOURCES defines and *_LDFLAGS for statically linked tests.
include Makefile.inc

if HAVE_LIB_STATIC
DOCKER_TESTS += $(DOCKER_TESTS_STATIC)
STANDALONE_TESTS += $(STANDALONE_TESTS_STATIC)
endif

# Some tests rely on the 'srcdir' env. Set by autotools automatically.
TESTS_ENVIRONMENT =

# Tests to run
TESTS = mansyntax.sh

if RUN_DOCKER_TESTS
TESTS += $(DOCKER_TESTS)
endif

TESTS += $(STANDALONE_TESTS)

# TAP tests
TEST_EXTENSIONS = .test
# 'TEST' in 'TEST_LOG_DRIVER' is the test extension in uppercase.
TEST_LOG_DRIVER = $(SHELL) $(top_srcdir)/tap-driver.sh

TESTS_ENVIRONMENT += EXEEXT=$(EXEEXT)
if RUN_SSHD_TESTS
if SSHD
TESTS += test_sshd.test
TESTS_ENVIRONMENT += SSHD="$(SSHD)"
endif
endif
if RUN_DOCKER_TESTS
TESTS += test_read_algos.test
endif

# Tests to build
check_PROGRAMS = $(DOCKER_TESTS) $(STANDALONE_TESTS) $(SSHD_TESTS)

# Internal library used by tests
LDADD = librunner.la
check_LTLIBRARIES = librunner.la

# This must be last in the list so it resolves symbols in previous libraries
LDADD += ../src/libssh2.la
