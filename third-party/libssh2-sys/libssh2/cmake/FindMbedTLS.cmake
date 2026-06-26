# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
#
###########################################################################
# Find the mbedtls library
#
# Input variables:
#
# MBEDTLS_INCLUDE_DIR   The mbedtls include directory
# MBEDCRYPTO_LIBRARY    Path to mbedcrypto library
#
# Result variables:
#
# MBEDTLS_FOUND         System has mbedtls
# MBEDTLS_INCLUDE_DIRS  The mbedtls include directories
# MBEDTLS_LIBRARIES     The mbedtls library names
# MBEDTLS_LIBRARY_DIRS  The mbedtls library directories
# MBEDTLS_CFLAGS        Required compiler flags
# MBEDTLS_VERSION       Version of mbedtls

if((UNIX OR VCPKG_TOOLCHAIN OR (MINGW AND NOT CMAKE_CROSSCOMPILING)) AND
   NOT DEFINED MBEDTLS_INCLUDE_DIR AND
   NOT DEFINED MBEDCRYPTO_LIBRARY)
  find_package(PkgConfig QUIET)
  pkg_check_modules(MBEDTLS "mbedcrypto")
endif()

if(MBEDTLS_FOUND)
  string(REPLACE ";" " " MBEDTLS_CFLAGS "${MBEDTLS_CFLAGS}")
  message(STATUS "Found MbedTLS (via pkg-config): ${MBEDTLS_INCLUDE_DIRS} (found version \"${MBEDTLS_VERSION}\")")
else()
  find_path(MBEDTLS_INCLUDE_DIR NAMES "mbedtls/version.h")
  find_library(MBEDCRYPTO_LIBRARY NAMES "mbedcrypto" "libmbedcrypto")

  if(MBEDTLS_INCLUDE_DIR)
    if(EXISTS "${MBEDTLS_INCLUDE_DIR}/mbedtls/build_info.h")  # 3.x
      set(_version_header "${MBEDTLS_INCLUDE_DIR}/mbedtls/build_info.h")
    elseif(EXISTS "${MBEDTLS_INCLUDE_DIR}/mbedtls/version.h")  # 2.x
      set(_version_header "${MBEDTLS_INCLUDE_DIR}/mbedtls/version.h")
    else()
      unset(_version_header)
    endif()
    if(_version_header)
      set(_version_regex "#[\t ]*define[\t ]+MBEDTLS_VERSION_STRING[\t ]+\"([0-9.]+)\"")
      file(STRINGS "${_version_header}" _version_str REGEX "${_version_regex}")
      string(REGEX REPLACE "${_version_regex}" "\\1" _version_str "${_version_str}")
      set(MBEDTLS_VERSION "${_version_str}")
      unset(_version_regex)
      unset(_version_str)
      unset(_version_header)
    endif()
  endif()

  include(FindPackageHandleStandardArgs)
  find_package_handle_standard_args(MbedTLS
    REQUIRED_VARS
      MBEDTLS_INCLUDE_DIR
      MBEDCRYPTO_LIBRARY
    VERSION_VAR
      MBEDTLS_VERSION
  )

  if(MBEDTLS_FOUND)
    set(MBEDTLS_INCLUDE_DIRS ${MBEDTLS_INCLUDE_DIR})
    set(MBEDTLS_LIBRARIES    ${MBEDCRYPTO_LIBRARY})
  endif()

  mark_as_advanced(MBEDTLS_INCLUDE_DIR MBEDCRYPTO_LIBRARY)
endif()
