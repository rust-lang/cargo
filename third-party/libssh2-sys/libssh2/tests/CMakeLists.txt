# Copyright (C) Alexander Lamaison <alexander.lamaison@gmail.com>
# Copyright (C) Viktor Szakats
#
# Redistribution and use in source and binary forms,
# with or without modification, are permitted provided
# that the following conditions are met:
#
#   Redistributions of source code must retain the above
#   copyright notice, this list of conditions and the
#   following disclaimer.
#
#   Redistributions in binary form must reproduce the above
#   copyright notice, this list of conditions and the following
#   disclaimer in the documentation and/or other materials
#   provided with the distribution.
#
#   Neither the name of the copyright holder nor the names
#   of any other contributors may be used to endorse or
#   promote products derived from this software without
#   specific prior written permission.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
# CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
# INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
# OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
# ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
# CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
# SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
# BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
# SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
# INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
# WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
# NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
# USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
# OF SUCH DAMAGE.
#
# SPDX-License-Identifier: BSD-3-Clause

include(CopyRuntimeDependencies)

list(APPEND LIBSSH2_LIBS ${LIBSSH2_LIBS_SOCKET})

transform_makefile_inc("Makefile.inc" "${CMAKE_CURRENT_BINARY_DIR}/Makefile.inc.cmake")
# Get 'DOCKER_TESTS', 'DOCKER_TESTS_STATIC', 'STANDALONE_TESTS', 'STANDALONE_TESTS_STATIC', 'SSHD_TESTS',
# 'librunner_la_SOURCES' variables
include("${CMAKE_CURRENT_BINARY_DIR}/Makefile.inc.cmake")
list(APPEND DOCKER_TESTS ${DOCKER_TESTS_STATIC})
list(APPEND STANDALONE_TESTS ${STANDALONE_TESTS_STATIC})

if(CMAKE_COMPILER_IS_GNUCC)
  find_program(GCOV_PATH "gcov")
  if(GCOV_PATH)
    set(_gcov_cflags "-g" "--coverage")
    if(NOT CMAKE_CXX_COMPILER_VERSION VERSION_LESS 8.0)
      set(_gcov_cflags "${_gcov_cflags} -fprofile-abs-path")
    endif()
  endif()
endif()

option(RUN_SSHD_TESTS "Run tests requiring sshd" ON)

find_program(SH_EXECUTABLE "sh")
mark_as_advanced(SH_EXECUTABLE)
if(SH_EXECUTABLE)
  if(RUN_SSHD_TESTS)
    find_program(SSHD_EXECUTABLE "sshd")
    mark_as_advanced(SSHD_EXECUTABLE)
  endif()

  add_test(NAME mansyntax COMMAND ${SH_EXECUTABLE} -c "${CMAKE_CURRENT_SOURCE_DIR}/mansyntax.sh")
endif()

add_library(runner STATIC ${librunner_la_SOURCES})
target_compile_definitions(runner PRIVATE "${CRYPTO_BACKEND_DEFINE}")
target_include_directories(runner PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/../src"
  "../src"
  "../include"
  "${CRYPTO_BACKEND_INCLUDE_DIR}")
target_link_libraries(runner PRIVATE libssh2)

foreach(_test IN LISTS DOCKER_TESTS STANDALONE_TESTS SSHD_TESTS)
  if(NOT ";${DOCKER_TESTS_STATIC};${STANDALONE_TESTS_STATIC};" MATCHES ";${_test};")
    set(_lib_for_tests ${LIB_SELECTED})
  elseif(TARGET ${LIB_STATIC})
    set(_lib_for_tests ${LIB_STATIC})
  else()
    unset(_lib_for_tests)
    message(STATUS "Skip test requiring static libssh2 lib: ${_test}")
  endif()

  # We support the same target as both Docker and SSHD test. Build those just once.
  # Skip building tests that require the static lib when the static lib is disabled.
  if(NOT TARGET ${_test} AND _lib_for_tests)
    add_executable(${_test} "${_test}.c")
    target_compile_definitions(${_test} PRIVATE "${CRYPTO_BACKEND_DEFINE}")
    target_include_directories(${_test} PRIVATE
      "${CMAKE_CURRENT_BINARY_DIR}/../src"
      "../src"
      "../include"
      "${CRYPTO_BACKEND_INCLUDE_DIR}")
    set_target_properties(${_test} PROPERTIES UNITY_BUILD OFF)

    # build a single test with gcov
    if(GCOV_PATH AND _test STREQUAL "test_auth_keyboard_info_request" AND TARGET ${LIB_STATIC})
      target_compile_options(${_test} BEFORE PRIVATE ${_gcov_cflags})
      target_link_libraries(${_test} runner ${_lib_for_tests} ${LIBSSH2_LIBS} "gcov")
    else()
      target_link_libraries(${_test} runner ${_lib_for_tests} ${LIBSSH2_LIBS})
    endif()

    list(APPEND TEST_TARGETS ${_test})
  endif()
endforeach()

option(RUN_DOCKER_TESTS "Run tests requiring Docker" ON)

if(RUN_DOCKER_TESTS)
  foreach(_test IN LISTS DOCKER_TESTS)
    if(TARGET ${_test})
      add_test(NAME ${_test} COMMAND "$<TARGET_FILE:${_test}>")
      set_property(TEST ${_test} APPEND PROPERTY ENVIRONMENT "srcdir=${CMAKE_CURRENT_SOURCE_DIR}")
    endif()
  endforeach()
endif()

foreach(_test IN LISTS STANDALONE_TESTS)
  if(TARGET ${_test})
    add_test(NAME ${_test} COMMAND "$<TARGET_FILE:${_test}>")
    set_property(TEST ${_test} APPEND PROPERTY ENVIRONMENT "srcdir=${CMAKE_CURRENT_SOURCE_DIR}")
  endif()
endforeach()

if(RUN_SSHD_TESTS AND SSHD_EXECUTABLE)
  unset(_sshd_test_targets)
  foreach(_test IN LISTS SSHD_TESTS)
    if(TARGET ${_test})
      set(_sshd_test_targets "${_sshd_test_targets} $<TARGET_FILE:${_test}>")
    endif()
  endforeach()

  if(_sshd_test_targets)
    add_test(NAME test_sshd COMMAND ${SH_EXECUTABLE} -c "${CMAKE_CURRENT_SOURCE_DIR}/test_sshd.test ${_sshd_test_targets}")
    set_property(TEST test_sshd APPEND PROPERTY ENVIRONMENT "srcdir=${CMAKE_CURRENT_SOURCE_DIR}")
    set_property(TEST test_sshd APPEND PROPERTY ENVIRONMENT "SSHD=${SSHD_EXECUTABLE}")
  endif()
endif()

if(RUN_DOCKER_TESTS)
  # CRYPT/MAC algo tests
  file(READ "test_read_algos.txt" _algo_tests)
  string(REGEX REPLACE "\\\n" ";" _algo_tests ${_algo_tests})
  foreach(_test IN LISTS _algo_tests)
    if(_test)
      set(_testname "test_read-${_test}")
      add_test(NAME ${_testname} COMMAND "$<TARGET_FILE:test_read>")
      set_property(TEST ${_testname} APPEND PROPERTY ENVIRONMENT "srcdir=${CMAKE_CURRENT_SOURCE_DIR}")
      if(_test MATCHES "mac-")
        set_property(TEST ${_testname} APPEND PROPERTY ENVIRONMENT "FIXTURE_TEST_MAC=${_test}")
      else()
        set_property(TEST ${_testname} APPEND PROPERTY ENVIRONMENT "FIXTURE_TEST_CRYPT=${_test}")
      endif()
    endif()
  endforeach()
endif()

add_custom_target(coverage
  COMMAND gcovr --root "${PROJECT_SOURCE_DIR}" --exclude tests/*
  COMMAND ${CMAKE_COMMAND} -E make_directory "${CMAKE_CURRENT_BINARY_DIR}/coverage"
  COMMAND gcovr --root "${PROJECT_SOURCE_DIR}" --exclude tests/* --html-details --output "${CMAKE_CURRENT_BINARY_DIR}/coverage/index.html")

add_custom_target(clean-coverage
  COMMAND rm -rf "${CMAKE_CURRENT_BINARY_DIR}/coverage")

add_target_to_copy_dependencies(
  TARGET copy_test_dependencies
  DEPENDENCIES ${RUNTIME_DEPENDENCIES}
  BEFORE_TARGETS ${TEST_TARGETS})
