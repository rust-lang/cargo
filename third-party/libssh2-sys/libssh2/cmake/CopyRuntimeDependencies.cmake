# Copyright (C) Alexander Lamaison <alexander.lamaison@gmail.com>
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

include(CMakeParseArguments)

function(add_target_to_copy_dependencies)
  set(options)
  set(oneValueArgs TARGET)
  set(multiValueArgs DEPENDENCIES BEFORE_TARGETS)
  cmake_parse_arguments(COPY "${options}" "${oneValueArgs}" "${multiValueArgs}" ${ARGN})

  if(NOT COPY_DEPENDENCIES)
    return()
  endif()

  # Using a custom target to drive custom commands stops multiple
  # parallel builds trying to kick off the commands at the same time
  add_custom_target(${COPY_TARGET})

  foreach(_target IN LISTS COPY_BEFORE_TARGETS)
    add_dependencies(${_target} ${COPY_TARGET})
  endforeach()

  foreach(_dependency IN LISTS COPY_DEPENDENCIES)
    add_custom_command(
      TARGET ${COPY_TARGET}
      DEPENDS ${_dependency}
      # Make directory first otherwise file is copied in place of
      # directory instead of into it
      COMMAND ${CMAKE_COMMAND} ARGS -E make_directory "${CMAKE_CURRENT_BINARY_DIR}/${CMAKE_CFG_INTDIR}"
      COMMAND ${CMAKE_COMMAND} ARGS -E copy ${_dependency} "${CMAKE_CURRENT_BINARY_DIR}/${CMAKE_CFG_INTDIR}"
      VERBATIM)
  endforeach()
endfunction()
