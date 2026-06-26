# Copyright (C) The libssh2 project and its contributors.
#
# SPDX-License-Identifier: BSD-3-Clause
#

# AC_PREREQ(2.59)
AC_INIT([libssh2],[-],[libssh2-devel@lists.haxx.se])
AC_CONFIG_MACRO_DIR([m4])
AC_CONFIG_SRCDIR([src])
AC_CONFIG_HEADERS([src/libssh2_config.h])
AC_REQUIRE_AUX_FILE([tap-driver.sh])
AM_MAINTAINER_MODE
m4_ifdef([AM_SILENT_RULES], [AM_SILENT_RULES([yes])])

dnl SED is needed by some of the tools
AC_PATH_PROG( SED, sed, sed-was-not-found-by-configure,
              $PATH:/usr/bin:/usr/local/bin)
AC_SUBST(SED)

if test "x$SED" = "xsed-was-not-found-by-configure"; then
  AC_MSG_WARN([sed was not found, this may ruin your chances to build fine])
fi

dnl figure out the libssh2 version
LIBSSH2_VERSION=`$SED -ne 's/^#define LIBSSH2_VERSION *"\(.*\)"/\1/p' ${srcdir}/include/libssh2.h`
AM_INIT_AUTOMAKE
AC_MSG_CHECKING([libssh2 version])
AC_MSG_RESULT($LIBSSH2_VERSION)

AC_SUBST(LIBSSH2_VERSION)

# Check for the OS.
# Daniel's note: this should not be necessary and we need to work to
# get this removed.
AC_CANONICAL_HOST
case "$host" in
    *-mingw*)
    LIBS="$LIBS -lws2_32"
    ;;
    *darwin*)
    ;;
    *hpux*)
    ;;
    *osf*)
    CFLAGS="$CFLAGS -D_POSIX_PII_SOCKET"
    ;;
    *)
    ;;
esac

dnl Our configure and build reentrant settings
CURL_CONFIGURE_REENTRANT

# Some systems (Solaris?) have socket() in -lsocket.
AC_SEARCH_LIBS(socket, socket)

# Solaris has inet_addr() in -lnsl.
AC_SEARCH_LIBS(inet_addr, nsl)

AC_SUBST(LIBS)

AC_PROG_CC
AC_PROG_CXX
AC_PROG_INSTALL
AC_PROG_LN_S
AC_PROG_MAKE_SET
AC_PATH_PROGS(SSHD, [sshd], [],
     [$PATH$PATH_SEPARATOR/usr/libexec$PATH_SEPARATOR]dnl
     [/usr/sbin$PATH_SEPARATOR/usr/etc$PATH_SEPARATOR/etc])
AM_CONDITIONAL(SSHD, test -n "$SSHD")
m4_ifdef([LT_INIT],
[dnl
LT_INIT([win32-dll])
],[dnl
AC_LIBTOOL_WIN32_DLL
AC_PROG_LIBTOOL
])
AC_C_BIGENDIAN

LT_LANG([Windows Resource])

dnl check for windows.h
case $host in
  *-*-msys | *-*-cygwin* | *-*-cegcc*)
    # These are POSIX-like systems using BSD-like sockets API.
    ;;
  *)
    AC_CHECK_HEADERS([windows.h], [have_windows_h=yes], [have_windows_h=no])
    ;;
esac

dnl check for how to do large files
AC_SYS_LARGEFILE

# Crypto backends

found_crypto=none
found_crypto_str=""
crypto_errors=""

m4_set_add([crypto_backends], [openssl])
m4_set_add([crypto_backends], [libgcrypt])
m4_set_add([crypto_backends], [mbedtls])
m4_set_add([crypto_backends], [wincng])
m4_set_add([crypto_backends], [wolfssl])

AC_ARG_WITH([crypto],
  AS_HELP_STRING([--with-crypto=auto|]m4_set_contents([crypto_backends], [|]),
    [Select crypto backend (default: auto)]),
  use_crypto=$withval,
  use_crypto=auto
)

case "${use_crypto}" in
  auto|m4_set_contents([crypto_backends], [|]))
    m4_set_map([crypto_backends], [LIBSSH2_CHECK_CRYPTO])
    ;;
  yes|"")
    crypto_errors="No crypto backend specified."
    ;;
  *)
    crypto_errors="Unknown crypto backend '${use_crypto}' specified."
    ;;
esac

if test "$found_crypto" = "none"; then
  crypto_errors="${crypto_errors}
Specify --with-crypto=\$backend and/or the necessary library search prefix.

Known crypto backends: auto, m4_set_contents([crypto_backends], [, ])"
  AS_MESSAGE([ERROR: ${crypto_errors}])
else
  test "$found_crypto_str" = "" && found_crypto_str="$found_crypto"
fi

# ECDSA support with WinCNG
AC_ARG_ENABLE(ecdsa-wincng,
  AS_HELP_STRING([--enable-ecdsa-wincng],
    WinCNG ECDSA support (requires Windows 10 or later)),
  [wincng_ecdsa=$enableval])
if test "$wincng_ecdsa" = yes; then
  AC_DEFINE(LIBSSH2_ECDSA_WINCNG, 1, [Enable WinCNG ECDSA support])
else
  wincng_ecdsa=no
fi

# libz

AC_ARG_WITH([libz],
  AS_HELP_STRING([--with-libz],[Use libz for compression]),
  use_libz=$withval,
  use_libz=auto)

found_libz=no
libz_errors=""

if test "$use_libz" != no; then
  AC_LIB_HAVE_LINKFLAGS([z], [], [#include <zlib.h>])
  if test "$ac_cv_libz" != yes; then
    if test "$use_libz" = auto; then
      AC_MSG_NOTICE([Cannot find libz, disabling compression])
      found_libz="disabled; no libz found"
    else
      libz_errors="No libz found.
Try --with-libz-prefix=PATH if you know that you have it."
      AS_MESSAGE([ERROR: $libz_errors])
    fi
  else
    AC_DEFINE(LIBSSH2_HAVE_ZLIB, 1, [Compile in zlib support])
    LIBSSH2_PC_REQUIRES_PRIVATE="$LIBSSH2_PC_REQUIRES_PRIVATE${LIBSSH2_PC_REQUIRES_PRIVATE:+,}zlib"
    found_libz="yes"
  fi
fi

AC_SUBST(LIBSSH2_PC_REQUIRES_PRIVATE)

#
# Optional Settings
#
AC_ARG_ENABLE(clear-memory,
  AS_HELP_STRING([--disable-clear-memory],[Disable clearing of memory before being freed]),
  [CLEAR_MEMORY=$enableval])
if test "$CLEAR_MEMORY" = "no"; then
  AC_DEFINE(LIBSSH2_NO_CLEAR_MEMORY, 1, [Disable clearing of memory before being freed])
  enable_clear_memory=no
else
  enable_clear_memory=yes
fi

LIBSSH2_CFLAG_EXTRAS=""

LIBSSH2_CHECK_OPTION_WERROR

dnl ************************************************************
dnl option to switch on compiler debug options
dnl
AC_MSG_CHECKING([whether to enable pedantic and debug compiler options])
AC_ARG_ENABLE(debug,
AS_HELP_STRING([--enable-debug],[Enable pedantic and debug options])
AS_HELP_STRING([--disable-debug],[Disable debug options]),
[ case "$enable_debug" in
  no)
       AC_MSG_RESULT(no)
       CPPFLAGS="$CPPFLAGS -DNDEBUG"
       ;;
  *)   AC_MSG_RESULT(yes)
    enable_debug=yes
    CPPFLAGS="$CPPFLAGS -DLIBSSH2DEBUG"
    CFLAGS="$CFLAGS -g"

    dnl set compiler "debug" options to become more picky, and remove
    dnl optimize options from CFLAGS
    CURL_CC_DEBUG_OPTS
       ;;
  esac
 ],
       enable_debug=no
       AC_MSG_RESULT(no)
)

AC_SUBST(LIBSSH2_CFLAG_EXTRAS)

dnl ************************************************************
dnl Enable hiding of internal symbols in library to reduce its size and
dnl speed dynamic linking of applications.  This currently is only supported
dnl on gcc >= 4.0 and SunPro C.
dnl
AC_MSG_CHECKING([whether to enable hidden symbols in the library])
AC_ARG_ENABLE(hidden-symbols,
AS_HELP_STRING([--enable-hidden-symbols],[Hide internal symbols in library])
AS_HELP_STRING([--disable-hidden-symbols],[Leave all symbols with default visibility in library (default)]),
[ case "$enableval" in
  no)
       AC_MSG_RESULT(no)
       ;;
  *)
       AC_MSG_CHECKING([whether $CC supports it])
       if test "$GCC" = yes ; then
         if $CC --help --verbose 2>&1 | grep fvisibility= > /dev/null ; then
           AC_MSG_RESULT(yes)
           AC_DEFINE(LIBSSH2_API, [__attribute__ ((visibility ("default")))], [to make a symbol visible])
           CFLAGS="$CFLAGS -fvisibility=hidden"
         else
            AC_MSG_RESULT(no)
          fi

       else
         dnl Test for SunPro cc
         if $CC 2>&1 | grep flags >/dev/null && $CC -flags | grep xldscope= >/dev/null ; then
           AC_MSG_RESULT(yes)
           AC_DEFINE(LIBSSH2_API, [__global], [to make a symbol visible])
           CFLAGS="$CFLAGS -xldscope=hidden"
         else
           AC_MSG_RESULT(no)
         fi
       fi
       ;;
  esac ],
       AC_MSG_RESULT(no)
)

dnl Build without deprecated APIs?
AC_ARG_ENABLE([deprecated],
  [AS_HELP_STRING([--disable-deprecated], [Build without deprecated APIs @<:@default=no@:>@])],
  [case "$enableval" in
    *)
      with_deprecated="no"
      CPPFLAGS="$CPPFLAGS -DLIBSSH2_NO_DEPRECATED"
      ;;
  esac],
  [with_deprecated="yes"])

# Run Docker tests?
AC_ARG_ENABLE([docker-tests],
  [AS_HELP_STRING([--disable-docker-tests],
    [Do not run tests requiring Docker])],
  [run_docker_tests=no], [run_docker_tests=yes])
AM_CONDITIONAL([RUN_DOCKER_TESTS], [test "x$run_docker_tests" != "xno"])

# Run sshd tests?
AC_ARG_ENABLE([sshd-tests],
  [AS_HELP_STRING([--disable-sshd-tests],
    [Do not run tests requiring sshd])],
  [run_sshd_tests=no], [run_sshd_tests=yes])
AM_CONDITIONAL([RUN_SSHD_TESTS], [test "x$run_sshd_tests" != "xno"])

# Build example applications?
AC_MSG_CHECKING([whether to build example applications])
AC_ARG_ENABLE([examples-build],
AS_HELP_STRING([--enable-examples-build], [Build example applications (this is the default)])
AS_HELP_STRING([--disable-examples-build], [Do not build example applications]),
[case "$enableval" in
  no | false)
    build_examples='no'
    ;;
  *)
    build_examples='yes'
    ;;
esac], [build_examples='yes'])
AC_MSG_RESULT($build_examples)
AM_CONDITIONAL([BUILD_EXAMPLES], [test "x$build_examples" != "xno"])


# Build OSS fuzzing targets?
AC_ARG_ENABLE([ossfuzzers],
  [AS_HELP_STRING([--enable-ossfuzzers],
    [Whether to generate the fuzzers for OSS-Fuzz])],
  [have_ossfuzzers=yes], [have_ossfuzzers=no])
AM_CONDITIONAL([USE_OSSFUZZERS], [test "x$have_ossfuzzers" = "xyes"])


# Set the correct flags for the given fuzzing engine.
AC_SUBST([LIB_FUZZING_ENGINE])
AM_CONDITIONAL([USE_OSSFUZZ_FLAG], [test "x$LIB_FUZZING_ENGINE" = "x-fsanitize=fuzzer"])
AM_CONDITIONAL([USE_OSSFUZZ_STATIC], [test -f "$LIB_FUZZING_ENGINE"])


# Checks for header files.
AC_CHECK_HEADERS([errno.h fcntl.h stdio.h unistd.h sys/uio.h])
AC_CHECK_HEADERS([sys/select.h sys/socket.h sys/ioctl.h sys/time.h])
AC_CHECK_HEADERS([arpa/inet.h netinet/in.h])
AC_CHECK_HEADERS([sys/un.h])

case $host in
  *darwin*|*interix*)
    dnl poll() does not work on these platforms
    dnl Interix: "does provide poll(), but the implementing developer must
    dnl have been in a bad mood, because poll() only works on the /proc
    dnl filesystem here"
    dnl macOS poll() has funny behaviors, like:
    dnl not being able to do poll on no fildescriptors (10.3?)
    dnl not being able to poll on some files (like anything in /dev)
    dnl not having reliable timeout support
    dnl inconsistent return of POLLHUP where other implementations give POLLIN
    AC_MSG_NOTICE([poll use is disabled on this platform])
    ;;
  *)
    AC_CHECK_FUNCS(poll)
    ;;
esac

AC_CHECK_FUNCS(gettimeofday select strtoll explicit_bzero explicit_memset memset_s snprintf)

dnl Check for select() into ws2_32 for Msys/Mingw
if test "$ac_cv_func_select" != "yes"; then
  AC_MSG_CHECKING([for select in ws2_32])
  AC_LINK_IFELSE([AC_LANG_PROGRAM([[
#ifdef HAVE_WINDOWS_H
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <winsock2.h>
#endif
    ]], [[
      select(0,(fd_set *)NULL,(fd_set *)NULL,(fd_set *)NULL,(struct timeval *)NULL);
    ]])],[
      AC_MSG_RESULT([yes])
      HAVE_SELECT="1"
      AC_DEFINE_UNQUOTED(HAVE_SELECT, 1,
        [Define to 1 if you have the select function.])
    ],[
      AC_MSG_RESULT([no])
  ])
fi

AC_FUNC_ALLOCA

# Checks for typedefs, structures, and compiler characteristics.
AC_C_CONST
AC_C_INLINE

CURL_CHECK_NONBLOCKING_SOCKET

missing_required_deps=0

if test "${libz_errors}" != ""; then
  AS_MESSAGE([ERROR: ${libz_errors}])
  missing_required_deps=1
fi

if test "$found_crypto" = "none"; then
  AS_MESSAGE([ERROR: ${crypto_errors}])
  missing_required_deps=1
fi

if test $missing_required_deps = 1; then
  AC_MSG_ERROR([Required dependencies are missing.])
fi

AM_CONDITIONAL([HAVE_WINDRES],
  [test "x$have_windows_h" = "xyes" && test "x${enable_shared}" = "xyes" && test -n "${RC}"])

AM_CONDITIONAL([HAVE_LIB_STATIC], [test "x$enable_static" != "xno"])

# Configure parameters

# Append crypto lib
if   test "$found_crypto" = "openssl"; then
  LIBS="${LIBS} ${LTLIBSSL}"
elif test "$found_crypto" = "wolfssl"; then
  LIBS="${LIBS} ${LTLIBWOLFSSL}"
elif test "$found_crypto" = "libgcrypt"; then
  LIBS="${LIBS} ${LTLIBGCRYPT}"
elif test "$found_crypto" = "wincng"; then
  LIBS="${LIBS} ${LTLIBBCRYPT}"
elif test "$found_crypto" = "mbedtls"; then
  LIBS="${LIBS} ${LTLIBMBEDCRYPTO}"
fi

LIBS="${LIBS} ${LTLIBZ}"

LIBSSH2_PC_LIBS_PRIVATE=$LIBS
AC_SUBST(LIBSSH2_PC_LIBS_PRIVATE)

dnl merge the pkg-config private fields into public ones when static-only
if test "x$enable_shared" = "xyes"; then
  LIBSSH2_PC_REQUIRES=
  LIBSSH2_PC_LIBS=
else
  LIBSSH2_PC_REQUIRES=$LIBSSH2_PC_REQUIRES_PRIVATE
  LIBSSH2_PC_LIBS=$LIBSSH2_PC_LIBS_PRIVATE
fi
AC_SUBST(LIBSSH2_PC_REQUIRES)
AC_SUBST(LIBSSH2_PC_LIBS)

AC_CONFIG_FILES([Makefile
                 src/Makefile
                 tests/Makefile
                 tests/ossfuzz/Makefile
                 example/Makefile
                 docs/Makefile
                 libssh2.pc])
AC_OUTPUT

AC_MSG_NOTICE([summary of build options:

  version:          ${LIBSSH2_VERSION}
  Host type:        ${host}
  Install prefix:   ${prefix}
  Compiler:         ${CC}
  Compiler flags:   ${CFLAGS}
  Library types:    Shared=${enable_shared}, Static=${enable_static}
  Crypto library:   ${found_crypto_str}
  WinCNG ECDSA:     $wincng_ecdsa
  zlib compression: ${found_libz}
  Clear memory:     $enable_clear_memory
  Deprecated APIs:  $with_deprecated
  Debug build:      $enable_debug
  Build examples:   $build_examples
  Run Docker tests: $run_docker_tests
  Run sshd tests:   $run_sshd_tests
  Path to sshd:     $ac_cv_path_SSHD (only for self-tests)
])
