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


# - check_function_exists_maybe_need_library(<function> <var> [lib1 ... libn])
#
# Check if function is available for linking, first without extra libraries, and
# then, if not found that way, linking in each optional library as well.  This
# function is similar to autotools AC_SEARCH_LIBS.
#
# If the function if found, this will define <var>.
#
# If the function was only found by linking in an additional library, this
# will define NEED_LIB_LIBX, where LIBX is the one of lib1 to libn that
# makes the function available, in uppercase.
#
# The following variables may be set before calling this macro to
# modify the way the check is run:
#
#  CMAKE_REQUIRED_FLAGS = string of compile command line flags
#  CMAKE_REQUIRED_DEFINITIONS = list of macros to define (-DFOO=bar)
#  CMAKE_REQUIRED_INCLUDES = list of include directories
#  CMAKE_REQUIRED_LIBRARIES = list of libraries to link
#

include(CheckFunctionExists)
include(CheckLibraryExists)

function(check_function_exists_may_need_library _function _variable)

  check_function_exists(${_function} ${_variable})

  if(NOT ${_variable})
    foreach(_lib IN LISTS ARGN)
      string(TOUPPER ${_lib} _up_lib)
      # Use new variable to prevent cache from previous step shortcircuiting
      # new test
      check_library_exists(${_lib} ${_function} "" HAVE_${_function}_IN_${_lib})
      if(HAVE_${_function}_IN_${_lib})
        set(${_variable} 1 CACHE INTERNAL "Function ${_function} found in library ${_lib}")
        set(NEED_LIB_${_up_lib} 1 CACHE INTERNAL "Need to link ${_lib}")
        break()
      endif()
    endforeach()
  endif()

endfunction()
