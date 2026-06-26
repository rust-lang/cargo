# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
include(CheckCSourceCompiles)

# - check_nonblocking_socket_support()
#
# Check for how to set a socket to non-blocking state. There seems to exist
# four known different ways, with the one used almost everywhere being POSIX
# and XPG3, while the other different ways for different systems (old BSD,
# Windows and Amiga).
#
# One of the following variables will be set indicating the supported
# method (if any):
#   HAVE_O_NONBLOCK
#   HAVE_FIONBIO
#   HAVE_IOCTLSOCKET_CASE
#   HAVE_SO_NONBLOCK
#
# The following variables may be set before calling this macro to
# modify the way the check is run:
#
#  CMAKE_REQUIRED_FLAGS = string of compile command line flags
#  CMAKE_REQUIRED_DEFINITIONS = list of macros to define (-DFOO=bar)
#  CMAKE_REQUIRED_INCLUDES = list of include directories
#  CMAKE_REQUIRED_LIBRARIES = list of libraries to link
#
macro(check_nonblocking_socket_support)
  # There are two known platforms (AIX 3.x and SunOS 4.1.x) where the
  # O_NONBLOCK define is found but does not work.
  check_c_source_compiles("
#include <sys/types.h>
#include <unistd.h>
#include <fcntl.h>

#if defined(sun) || defined(__sun__) || defined(__SUNPRO_C) || defined(__SUNPRO_CC)
# if defined(__SVR4) || defined(__srv4__)
#  define PLATFORM_SOLARIS
# else
#  define PLATFORM_SUNOS4
# endif
#endif
#if (defined(_AIX) || defined(__xlC__)) && !defined(_AIX41)
# define PLATFORM_AIX_V3
#endif

#if defined(PLATFORM_SUNOS4) || defined(PLATFORM_AIX_V3) || defined(__BEOS__)
#error \"O_NONBLOCK does not work on this platform\"
#endif

int main(void)
{
    int socket = 0;
    (void)fcntl(socket, F_SETFL, O_NONBLOCK);
}"
    HAVE_O_NONBLOCK)

  if(NOT HAVE_O_NONBLOCK)
    check_c_source_compiles("/* FIONBIO test (old-style unix) */
#include <unistd.h>
#include <stropts.h>

int main(void)
{
    int socket = 0;
    int flags = 0;
    (void)ioctl(socket, FIONBIO, &flags);
}"
      HAVE_FIONBIO)

    if(NOT HAVE_FIONBIO)
      check_c_source_compiles("/* IoctlSocket test (Amiga?) */
#include <sys/ioctl.h>

int main(void)
{
    int socket = 0;
    (void)IoctlSocket(socket, FIONBIO, (long)1);
}"
        HAVE_IOCTLSOCKET_CASE)

      if(NOT HAVE_IOCTLSOCKET_CASE)
        check_c_source_compiles("/* SO_NONBLOCK test (BeOS) */
#include <socket.h>

int main(void)
{
    long b = 1;
    int socket = 0;
    (void)setsockopt(socket, SOL_SOCKET, SO_NONBLOCK, &b, sizeof(b));
}"
          HAVE_SO_NONBLOCK)
      endif()
    endif()
  endif()
endmacro()
