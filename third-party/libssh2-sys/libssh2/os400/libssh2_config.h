/*
 * Copyright (C) Patrick Monnerat <patrick@monnerat.net>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms,
 * with or without modification, are permitted provided
 * that the following conditions are met:
 *
 *   Redistributions of source code must retain the above
 *   copyright notice, this list of conditions and the
 *   following disclaimer.
 *
 *   Redistributions in binary form must reproduce the above
 *   copyright notice, this list of conditions and the following
 *   disclaimer in the documentation and/or other materials
 *   provided with the distribution.
 *
 *   Neither the name of the copyright holder nor the names
 *   of any other contributors may be used to endorse or
 *   promote products derived from this software without
 *   specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
 * CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
 * INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
 * WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
 * NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
 * USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
 * OF SUCH DAMAGE.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#ifndef LIBSSH2_CONFIG_H
#define LIBSSH2_CONFIG_H

/* Define if building universal (internal helper macro) */
#undef AC_APPLE_UNIVERSAL_BUILD

/* Define to one of `_getb67', `GETB67', `getb67' for Cray-2 and Cray-YMP
   systems. This function is required for `alloca.c' support on those systems.
*/
#undef CRAY_STACKSEG_END

/* Define to 1 if using `alloca.c'. */
#undef C_ALLOCA

/* Define to 1 if you have `alloca', as a function or macro. */
#define HAVE_ALLOCA 1

/* Define to 1 if you have <alloca.h> and it should be used (not on Ultrix). */
#define HAVE_ALLOCA_H 1

/* Define to 1 if you have the <arpa/inet.h> header file. */
#define HAVE_ARPA_INET_H 1

/* use FIONBIO for non-blocking sockets */
#undef HAVE_FIONBIO

/* Define to 1 if you have the `gettimeofday' function. */
#define HAVE_GETTIMEOFDAY 1

/* Define to 1 if you have the <inttypes.h> header file. */
#define HAVE_INTTYPES_H 1

/* use Ioctlsocket() for non-blocking sockets */
#undef HAVE_IOCTLSOCKET_CASE

/* Define if you have the gcrypt library. */
#undef HAVE_LIBGCRYPT

/* Define to 1 if you have the <netinet/in.h> header file. */
#define HAVE_NETINET_IN_H 1

/* use O_NONBLOCK for non-blocking sockets */
#define HAVE_O_NONBLOCK 1

/* Define to 1 if you have the `poll' function. */
#undef HAVE_POLL

/* Define to 1 if you have the select function. */
#define HAVE_SELECT 1

/* use SO_NONBLOCK for non-blocking sockets */
#undef HAVE_SO_NONBLOCK

/* Define to 1 if you have the <stdio.h> header file. */
#define HAVE_STDIO_H 1

/* Define to 1 if you have the `strtoll' function. */
#define HAVE_STRTOLL 1

/* Define to 1 if you have the <sys/ioctl.h> header file. */
#define HAVE_SYS_IOCTL_H 1

/* Define to 1 if you have the <sys/select.h> header file. */
#undef HAVE_SYS_SELECT_H

/* Define to 1 if you have the <sys/socket.h> header file. */
#define HAVE_SYS_SOCKET_H 1

/* Define to 1 if you have the <sys/time.h> header file. */
#define HAVE_SYS_TIME_H 1

/* Define to 1 if you have the <sys/uio.h> header file. */
#define HAVE_SYS_UIO_H 1

/* Define to 1 if you have the <sys/un.h> header file. */
#define HAVE_SYS_UN_H 1

/* Define to 1 if you have the <unistd.h> header file. */
#define HAVE_UNISTD_H 1

/* to make a symbol visible */
#undef LIBSSH2_API

/* Compile in zlib support */
/* #undef LIBSSH2_HAVE_ZLIB */

/* Use libgcrypt */
#undef LIBSSH2_LIBGCRYPT

/* Use OpenSSL */
#undef LIBSSH2_OPENSSL

/* Use Windows CNG */
#undef LIBSSH2_WINCNG

/* Use OS/400 Qc3 */
#define LIBSSH2_OS400QC3

/* Use our platform-specific local implementation:
   _libssh2_os400_snprintf */
#define HAVE_SNPRINTF 1

/* Define to the sub-directory in which libtool stores uninstalled libraries.
*/
#define LT_OBJDIR ".libs/"

/* Define to 1 if _REENTRANT preprocessor symbol must be defined. */
#undef NEED_REENTRANT

/* Name of package */
#define PACKAGE "libssh2"

/* Define to the address where bug reports for this package should be sent. */
#define PACKAGE_BUGREPORT "libssh2-devel@lists.haxx.se"

/* Define to the full name of this package. */
#define PACKAGE_NAME "libssh2"

/* Define to the full name and version of this package. */
#define PACKAGE_STRING "libssh2 -"

/* Define to the one symbol short name of this package. */
#define PACKAGE_TARNAME "libssh2"

/* Define to the home page for this package. */
#define PACKAGE_URL ""

/* Define to the version of this package. */
#define PACKAGE_VERSION "-"

/* If using the C implementation of alloca, define if you know the
   direction of stack growth for your system; otherwise it will be
   automatically deduced at runtime.
     STACK_DIRECTION > 0 => grows toward higher addresses
     STACK_DIRECTION < 0 => grows toward lower addresses
     STACK_DIRECTION = 0 => direction of growth unknown */
#undef STACK_DIRECTION

/* Define to 1 if you have the ANSI C header files. */
#define STDC_HEADERS 1

/* Version number of package */
#define VERSION "-"

/* Define WORDS_BIGENDIAN to 1 if your processor stores words with the most
   significant byte first (like Motorola and SPARC, unlike Intel). */
#define WORDS_BIGENDIAN 1

/* Number of bits in a file offset, on hosts where this is settable. */
#undef _FILE_OFFSET_BITS

/* Define for large files, on AIX-style hosts. */
#undef _LARGE_FILES

/* Define to empty if `const' does not conform to ANSI C. */
#undef const

/* Define to `__inline__' or `__inline' if that's what the C compiler
   calls it, or to nothing if 'inline' is not supported under any name.  */
#ifndef __cplusplus
#define inline
#endif

/* Define to `unsigned int' if <sys/types.h> does not define. */
#undef size_t


#ifndef LIBSSH2_DISABLE_QADRT_EXT
/* Remap zlib procedures to ASCII versions. */
#pragma map(inflateInit_, "_libssh2_os400_inflateInit_")
#pragma map(deflateInit_, "_libssh2_os400_deflateInit_")
#endif

#endif
/* vim: set expandtab ts=4 sw=4: */
