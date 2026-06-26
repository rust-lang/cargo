# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
dnl CURL_CPP_P
dnl
dnl Check if $cpp -P should be used for extract define values due to gcc 5
dnl splitting up strings and defines between line outputs. gcc by default
dnl (without -P) will show TEST EINVAL TEST as
dnl
dnl # 13 "conftest.c"
dnl TEST
dnl # 13 "conftest.c" 3 4
dnl     22
dnl # 13 "conftest.c"
dnl            TEST

AC_DEFUN([CURL_CPP_P], [
  AC_MSG_CHECKING([if cpp -P is needed])
  AC_EGREP_CPP([TEST.*TEST], [
 #include <errno.h>
TEST EINVAL TEST
  ], [cpp=no], [cpp=yes])
  AC_MSG_RESULT([$cpp])

  dnl we need cpp -P so check if it works then
  if test "x$cpp" = "xyes"; then
    AC_MSG_CHECKING([if cpp -P works])
    OLDCPPFLAGS=$CPPFLAGS
    CPPFLAGS="$CPPFLAGS -P"
    AC_EGREP_CPP([TEST.*TEST], [
 #include <errno.h>
TEST EINVAL TEST
    ], [cpp_p=yes], [cpp_p=no])
    AC_MSG_RESULT([$cpp_p])

    if test "x$cpp_p" = "xno"; then
      AC_MSG_WARN([failed to figure out cpp -P alternative])
      # without -P
      CPPPFLAG=""
    else
      # with -P
      CPPPFLAG="-P"
    fi
    dnl restore CPPFLAGS
    CPPFLAGS=$OLDCPPFLAGS
  else
    # without -P
    CPPPFLAG=""
  fi
])

dnl CURL_CHECK_DEF (SYMBOL, [INCLUDES], [SILENT])
dnl -------------------------------------------------
dnl Use the C preprocessor to find out if the given object-style symbol
dnl is defined and get its expansion. This macro will not use default
dnl includes even if no INCLUDES argument is given. This macro will run
dnl silently when invoked with three arguments. If the expansion would
dnl result in a set of double-quoted strings the returned expansion will
dnl actually be a single double-quoted string concatenating all them.

AC_DEFUN([CURL_CHECK_DEF], [
  AC_REQUIRE([CURL_CPP_P])dnl
  OLDCPPFLAGS=$CPPFLAGS
  # CPPPFLAG comes from CURL_CPP_P
  CPPFLAGS="$CPPFLAGS $CPPPFLAG"
  AS_VAR_PUSHDEF([ac_HaveDef], [curl_cv_have_def_$1])dnl
  AS_VAR_PUSHDEF([ac_Def], [curl_cv_def_$1])dnl
  if test -z "$SED"; then
    AC_MSG_ERROR([SED not set. Cannot continue without SED being set.])
  fi
  if test -z "$GREP"; then
    AC_MSG_ERROR([GREP not set. Cannot continue without GREP being set.])
  fi
  ifelse($3,,[AC_MSG_CHECKING([for preprocessor definition of $1])])
  tmp_exp=""
  AC_PREPROC_IFELSE([
    AC_LANG_SOURCE(
ifelse($2,,,[$2])[[
#ifdef $1
CURL_DEF_TOKEN $1
#endif
    ]])
  ],[
    tmp_exp=`eval "$ac_cpp conftest.$ac_ext" 2>/dev/null | \
      "$GREP" CURL_DEF_TOKEN 2>/dev/null | \
      "$SED" 's/.*CURL_DEF_TOKEN[[ ]][[ ]]*//' 2>/dev/null | \
      "$SED" 's/[["]][[ ]]*[["]]//g' 2>/dev/null`
    if test -z "$tmp_exp" || test "$tmp_exp" = "$1"; then
      tmp_exp=""
    fi
  ])
  if test -z "$tmp_exp"; then
    AS_VAR_SET(ac_HaveDef, no)
    ifelse($3,,[AC_MSG_RESULT([no])])
  else
    AS_VAR_SET(ac_HaveDef, yes)
    AS_VAR_SET(ac_Def, $tmp_exp)
    ifelse($3,,[AC_MSG_RESULT([$tmp_exp])])
  fi
  AS_VAR_POPDEF([ac_Def])dnl
  AS_VAR_POPDEF([ac_HaveDef])dnl
  CPPFLAGS=$OLDCPPFLAGS
])

dnl CURL_CHECK_COMPILER_CLANG
dnl -------------------------------------------------
dnl Verify if compiler being used is clang.

AC_DEFUN([CURL_CHECK_COMPILER_CLANG], [
  AC_BEFORE([$0],[CURL_CHECK_COMPILER_GNU_C])dnl
  AC_MSG_CHECKING([if compiler is clang])
  CURL_CHECK_DEF([__clang__], [], [silent])
  if test "$curl_cv_have_def___clang__" = "yes"; then
    AC_MSG_RESULT([yes])
    AC_MSG_CHECKING([if compiler is xlclang])
    CURL_CHECK_DEF([__ibmxl__], [], [silent])
    if test "$curl_cv_have_def___ibmxl__" = "yes" ; then
      dnl IBM's almost-compatible clang version
      AC_MSG_RESULT([yes])
      compiler_id="XLCLANG"
    else
      AC_MSG_RESULT([no])
      compiler_id="CLANG"
    fi
    flags_dbg_yes="-g"
    flags_opt_all="-O -O0 -O1 -O2 -Os -O3 -O4"
    flags_opt_yes="-O2"
    flags_opt_off="-O0"
  else
    AC_MSG_RESULT([no])
  fi
])

dnl **********************************************************************
dnl CURL_DETECT_ICC ([ACTION-IF-YES])
dnl
dnl check if this is the Intel ICC compiler, and if so run the ACTION-IF-YES
dnl sets the $ICC variable to "yes" or "no"
dnl **********************************************************************
AC_DEFUN([CURL_DETECT_ICC],
[
  ICC="no"
  AC_MSG_CHECKING([for icc in use])
  if test "$GCC" = "yes"; then
    dnl check if this is icc acting as gcc in disguise
    AC_EGREP_CPP([^__INTEL_COMPILER], [__INTEL_COMPILER],
      dnl action if the text is found, this it has not been replaced by the
      dnl cpp
      ICC="no",
      dnl the text was not found, it was replaced by the cpp
      ICC="yes"
      AC_MSG_RESULT([yes])
      [$1]
    )
  fi
  if test "$ICC" = "no"; then
    # this is not ICC
    AC_MSG_RESULT([no])
  fi
])

dnl We create a function for detecting which compiler we use and then set as
dnl pedantic compiler options as possible for that particular compiler. The
dnl options are only used for debug-builds.

AC_DEFUN([CURL_CC_DEBUG_OPTS],
[
  if test "z$CLANG" = "z"; then
    CURL_CHECK_COMPILER_CLANG
    if test "z$compiler_id" = "zCLANG"; then
      CLANG="yes"
    else
      CLANG="no"
    fi
  fi
  if test "z$ICC" = "z"; then
    CURL_DETECT_ICC
  fi

  if test "$CLANG" = "yes"; then

          # indentation to match curl's m4/curl-compilers.m4

          dnl figure out clang version!
          AC_MSG_CHECKING([compiler version])
          fullclangver=`$CC -v 2>&1 | grep version`
          if echo $fullclangver | grep 'Apple' >/dev/null; then
            appleclang=1
          else
            appleclang=0
          fi
          clangver=`echo $fullclangver | grep "based on LLVM " | "$SED" 's/.*(based on LLVM \(@<:@0-9@:>@*\.@<:@0-9@:>@*\).*)/\1/'`
          if test -z "$clangver"; then
            clangver=`echo $fullclangver | "$SED" 's/.*version \(@<:@0-9@:>@*\.@<:@0-9@:>@*\).*/\1/'`
            oldapple=0
          else
            oldapple=1
          fi
          clangvhi=`echo $clangver | cut -d . -f1`
          clangvlo=`echo $clangver | cut -d . -f2`
          compiler_num=`(expr $clangvhi "*" 100 + $clangvlo) 2>/dev/null`
          if test "$appleclang" = '1' && test "$oldapple" = '0'; then
            dnl Starting with Xcode 7 / clang 3.7, Apple clang won't tell its upstream version
            if   test "$compiler_num" -ge '1300'; then compiler_num='1200'
            elif test "$compiler_num" -ge '1205'; then compiler_num='1101'
            elif test "$compiler_num" -ge '1204'; then compiler_num='1000'
            elif test "$compiler_num" -ge '1107'; then compiler_num='900'
            elif test "$compiler_num" -ge '1103'; then compiler_num='800'
            elif test "$compiler_num" -ge '1003'; then compiler_num='700'
            elif test "$compiler_num" -ge '1001'; then compiler_num='600'
            elif test "$compiler_num" -ge  '904'; then compiler_num='500'
            elif test "$compiler_num" -ge  '902'; then compiler_num='400'
            elif test "$compiler_num" -ge  '803'; then compiler_num='309'
            elif test "$compiler_num" -ge  '703'; then compiler_num='308'
            else                                       compiler_num='307'
            fi
          fi
          AC_MSG_RESULT([clang '$compiler_num' (raw: '$fullclangver' / '$clangver')])

          tmp_CFLAGS="-pedantic"
          if test "$want_werror" = "yes"; then
            LIBSSH2_CFLAG_EXTRAS="$LIBSSH2_CFLAG_EXTRAS -pedantic-errors"
          fi
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [all extra])
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [pointer-arith write-strings])
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [shadow])
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [inline nested-externs])
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-declarations])
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-prototypes])
          tmp_CFLAGS="$tmp_CFLAGS -Wno-long-long"
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [float-equal])
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [sign-compare])
          tmp_CFLAGS="$tmp_CFLAGS -Wno-multichar"
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [undef])
          tmp_CFLAGS="$tmp_CFLAGS -Wno-format-nonliteral"
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [endif-labels strict-prototypes])
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [declaration-after-statement])
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [cast-align])
          tmp_CFLAGS="$tmp_CFLAGS -Wno-system-headers"
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [shorten-64-to-32])
          #
          dnl Only clang 1.1 or later
          if test "$compiler_num" -ge "101"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unused])
          fi
          #
          dnl Only clang 2.7 or later
          if test "$compiler_num" -ge "207"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [address])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [attributes])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [bad-function-cast])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [conversion])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [div-by-zero format-security])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [empty-body])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-field-initializers])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-noreturn])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [old-style-definition])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [redundant-decls])
          # CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [switch-enum])       # Not used because this basically disallows default case
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [type-limits])
            if test "x$have_windows_h" != "xyes"; then
              CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unused-macros])  # Seen to clash with libtool-generated stub code
            fi
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unreachable-code unused-parameter])
          fi
          #
          dnl Only clang 2.8 or later
          if test "$compiler_num" -ge "208"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [ignored-qualifiers])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [vla])
          fi
          #
          dnl Only clang 2.9 or later
          if test "$compiler_num" -ge "209"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [sign-conversion])
            tmp_CFLAGS="$tmp_CFLAGS -Wno-error=sign-conversion"           # FIXME
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [shift-sign-overflow])
          # CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [padded])  # Not used because we cannot change public structs
          fi
          #
          dnl Only clang 3.0 or later
          if test "$compiler_num" -ge "300"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [language-extension-token])
            tmp_CFLAGS="$tmp_CFLAGS -Wformat=2"
          fi
          #
          dnl Only clang 3.2 or later
          if test "$compiler_num" -ge "302"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [enum-conversion])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [sometimes-uninitialized])
            case $host_os in
            cygwin* | mingw*)
              dnl skip missing-variable-declarations warnings for cygwin and
              dnl mingw because the libtool wrapper executable causes them
              ;;
            *)
              CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-variable-declarations])
              ;;
            esac
          fi
          #
          dnl Only clang 3.4 or later
          if test "$compiler_num" -ge "304"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [header-guard])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unused-const-variable])
          fi
          #
          dnl Only clang 3.5 or later
          if test "$compiler_num" -ge "305"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [pragmas])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unreachable-code-break])
          fi
          #
          dnl Only clang 3.6 or later
          if test "$compiler_num" -ge "306"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [double-promotion])
          fi
          #
          dnl Only clang 3.9 or later
          if test "$compiler_num" -ge "309"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [comma])
            # avoid the varargs warning, fixed in 4.0
            # https://bugs.llvm.org/show_bug.cgi?id=29140
            if test "$compiler_num" -lt "400"; then
              tmp_CFLAGS="$tmp_CFLAGS -Wno-varargs"
            fi
          fi
          dnl clang 7 or later
          if test "$compiler_num" -ge "700"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [assign-enum])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [extra-semi-stmt])
          fi
          dnl clang 10 or later
          if test "$compiler_num" -ge "1000"; then
            tmp_CFLAGS="$tmp_CFLAGS -Wimplicit-fallthrough"  # we have silencing markup for clang 10.0 and above only
          fi

          CFLAGS="$CFLAGS $tmp_CFLAGS"

          AC_MSG_NOTICE([Added this set of compiler options: $tmp_CFLAGS])

  elif test "$GCC" = "yes"; then

        # indentation to match curl's m4/curl-compilers.m4

        dnl figure out gcc version!
        AC_MSG_CHECKING([compiler version])
        # strip '-suffix' parts, e.g. Ubuntu Windows cross-gcc returns '10-win32'
        gccver=`$CC -dumpversion | sed -E 's/-.+$//'`
        gccvhi=`echo $gccver | cut -d . -f1`
        if echo $gccver | grep -F "." >/dev/null; then
          gccvlo=`echo $gccver | cut -d . -f2`
        else
          gccvlo="0"
        fi
        compiler_num=`(expr $gccvhi "*" 100 + $gccvlo) 2>/dev/null`
        AC_MSG_RESULT([gcc '$compiler_num' (raw: '$gccver')])

        if test "$ICC" = "yes"; then
          dnl this is icc, not gcc.

          dnl ICC warnings we ignore:
          dnl * 269 warns on our "%Od" printf formatters for curl_off_t output:
          dnl   "invalid format string conversion"
          dnl * 279 warns on static conditions in while expressions
          dnl * 981 warns on "operands are evaluated in unspecified order"
          dnl * 1418 "external definition with no prior declaration"
          dnl * 1419 warns on "external declaration in primary source file"
          dnl   which we know and do on purpose.

          tmp_CFLAGS="-wd279,269,981,1418,1419"

          if test "$compiler_num" -gt "600"; then
             dnl icc 6.0 and older doesn't have the -Wall flag
             tmp_CFLAGS="-Wall $tmp_CFLAGS"
          fi
        else dnl $ICC = yes
          dnl this is a set of options we believe *ALL* gcc versions support:
          tmp_CFLAGS="-pedantic"
          if test "$want_werror" = "yes"; then
            LIBSSH2_CFLAG_EXTRAS="$LIBSSH2_CFLAG_EXTRAS -pedantic-errors"
          fi
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [all])
          tmp_CFLAGS="$tmp_CFLAGS -W"
          CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [pointer-arith write-strings])
          #
          dnl Only gcc 2.7 or later
          if test "$compiler_num" -ge "207"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [inline nested-externs])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-declarations])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-prototypes])
          fi
          #
          dnl Only gcc 2.95 or later
          if test "$compiler_num" -ge "295"; then
            tmp_CFLAGS="$tmp_CFLAGS -Wno-long-long"
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [bad-function-cast])
          fi
          #
          dnl Only gcc 2.96 or later
          if test "$compiler_num" -ge "296"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [float-equal])
            tmp_CFLAGS="$tmp_CFLAGS -Wno-multichar"
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [sign-compare])
            dnl -Wundef used only if gcc is 2.96 or later since we get
            dnl lots of "`_POSIX_C_SOURCE' is not defined" in system
            dnl headers with gcc 2.95.4 on FreeBSD 4.9
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [undef])
          fi
          #
          dnl Only gcc 2.97 or later
          if test "$compiler_num" -ge "297"; then
            tmp_CFLAGS="$tmp_CFLAGS -Wno-format-nonliteral"
          fi
          #
          dnl Only gcc 3.0 or later
          if test "$compiler_num" -ge "300"; then
            tmp_CFLAGS="$tmp_CFLAGS -Wno-system-headers"
            dnl -Wunreachable-code seems totally unreliable on my gcc 3.3.2 on
            dnl on i686-Linux as it gives us heaps with false positives.
            dnl Also, on gcc 4.0.X it is totally unbearable and complains all
            dnl over making it unusable for generic purposes. Let's not use it.
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unused shadow])
          fi
          #
          dnl Only gcc 3.3 or later
          if test "$compiler_num" -ge "303"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [endif-labels strict-prototypes])
          fi
          #
          dnl Only gcc 3.4 or later
          if test "$compiler_num" -ge "304"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [declaration-after-statement])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [old-style-definition])
          fi
          #
          dnl Only gcc 4.0 or later
          if test "$compiler_num" -ge "400"; then
            tmp_CFLAGS="$tmp_CFLAGS -Wstrict-aliasing=3"
          fi
          #
          dnl Only gcc 4.1 or later (possibly earlier)
          if test "$compiler_num" -ge "401"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [attributes])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [div-by-zero format-security])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-field-initializers])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-noreturn])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unreachable-code unused-parameter])
          # CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [padded])           # Not used because we cannot change public structs
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [pragmas])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [redundant-decls])
          # CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [switch-enum])      # Not used because this basically disallows default case
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unused-macros])
          fi
          #
          dnl Only gcc 4.2 or later
          if test "$compiler_num" -ge "402"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [cast-align])
          fi
          #
          dnl Only gcc 4.3 or later
          if test "$compiler_num" -ge "403"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [address])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [type-limits old-style-declaration])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [missing-parameter-type empty-body])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [clobbered ignored-qualifiers])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [conversion trampolines])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [sign-conversion])
            tmp_CFLAGS="$tmp_CFLAGS -Wno-error=sign-conversion"          # FIXME
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [vla])
            dnl required for -Warray-bounds, included in -Wall
            tmp_CFLAGS="$tmp_CFLAGS -ftree-vrp"
          fi
          #
          dnl Only gcc 4.5 or later
          if test "$compiler_num" -ge "405"; then
            dnl Only windows targets
            case $host_os in
            mingw*)
              tmp_CFLAGS="$tmp_CFLAGS -Wno-pedantic-ms-format"
              ;;
            esac
          fi
          #
          dnl Only gcc 4.6 or later
          if test "$compiler_num" -ge "406"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [double-promotion])
          fi
          #
          dnl only gcc 4.8 or later
          if test "$compiler_num" -ge "408"; then
            tmp_CFLAGS="$tmp_CFLAGS -Wformat=2"
          fi
          #
          dnl Only gcc 5 or later
          if test "$compiler_num" -ge "500"; then
            tmp_CFLAGS="$tmp_CFLAGS -Warray-bounds=2"
          fi
          #
          dnl Only gcc 6 or later
          if test "$compiler_num" -ge "600"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [shift-negative-value])
            tmp_CFLAGS="$tmp_CFLAGS -Wshift-overflow=2"
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [null-dereference])
            tmp_CFLAGS="$tmp_CFLAGS -fdelete-null-pointer-checks"
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [duplicated-cond])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [unused-const-variable])
          fi
          #
          dnl Only gcc 7 or later
          if test "$compiler_num" -ge "700"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [duplicated-branches])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [restrict])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [alloc-zero])
            tmp_CFLAGS="$tmp_CFLAGS -Wformat-overflow=2"
            tmp_CFLAGS="$tmp_CFLAGS -Wformat-truncation=2"
            tmp_CFLAGS="$tmp_CFLAGS -Wimplicit-fallthrough"
          fi
          #
          dnl Only gcc 10 or later
          if test "$compiler_num" -ge "1000"; then
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [arith-conversion])
            CURL_ADD_COMPILER_WARNINGS([tmp_CFLAGS], [enum-conversion])
          fi

          for flag in $CPPFLAGS; do
            case "$flag" in
             -I*)
               dnl Include path, provide a -isystem option for the same dir
               dnl to prevent warnings in those dirs. The -isystem was not very
               dnl reliable on earlier gcc versions.
               add=`echo $flag | sed 's/^-I/-isystem /g'`
               tmp_CFLAGS="$tmp_CFLAGS $add"
               ;;
            esac
          done

    fi dnl $ICC = no

    CFLAGS="$CFLAGS $tmp_CFLAGS"

    AC_MSG_NOTICE([Added this set of compiler options: $tmp_CFLAGS])

  else dnl $GCC = yes

    AC_MSG_NOTICE([Added no extra compiler options])

  fi dnl $GCC = yes

  dnl strip off optimizer flags
  NEWFLAGS=""
  for flag in $CFLAGS; do
    case "$flag" in
    -O*)
      dnl echo "cut off $flag"
      ;;
    *)
      NEWFLAGS="$NEWFLAGS $flag"
      ;;
    esac
  done
  CFLAGS=$NEWFLAGS

]) dnl end of AC_DEFUN()

dnl CURL_ADD_COMPILER_WARNINGS (WARNING-LIST, NEW-WARNINGS)
dnl -------------------------------------------------------
dnl Contents of variable WARNING-LIST and NEW-WARNINGS are
dnl handled as whitespace separated lists of words.
dnl Add each compiler warning from NEW-WARNINGS that has not
dnl been disabled via CFLAGS to WARNING-LIST.

AC_DEFUN([CURL_ADD_COMPILER_WARNINGS], [
  AC_REQUIRE([CURL_SHFUNC_SQUEEZE])dnl
  ac_var_added_warnings=""
  for warning in [$2]; do
    CURL_VAR_MATCH(CFLAGS, [-Wno-$warning -W$warning])
    if test "$ac_var_match_word" = "no"; then
      ac_var_added_warnings="$ac_var_added_warnings -W$warning"
    fi
  done
  dnl squeeze whitespace out of result
  [$1]="$[$1] $ac_var_added_warnings"
  squeeze [$1]
])

dnl CURL_SHFUNC_SQUEEZE
dnl -------------------------------------------------
dnl Declares a shell function squeeze() which removes
dnl redundant whitespace out of a shell variable.

AC_DEFUN([CURL_SHFUNC_SQUEEZE], [
squeeze() {
  _sqz_result=""
  eval _sqz_input=\[$][$]1
  for _sqz_token in $_sqz_input; do
    if test -z "$_sqz_result"; then
      _sqz_result="$_sqz_token"
    else
      _sqz_result="$_sqz_result $_sqz_token"
    fi
  done
  eval [$]1=\$_sqz_result
  return 0
}
])

dnl CURL_VAR_MATCH (VARNAME, VALUE)
dnl -------------------------------------------------
dnl Verifies if shell variable VARNAME contains VALUE.
dnl Contents of variable VARNAME and VALUE are handled
dnl as whitespace separated lists of words. If at least
dnl one word of VALUE is present in VARNAME the match
dnl is considered positive, otherwise false.

AC_DEFUN([CURL_VAR_MATCH], [
  ac_var_match_word="no"
  for word1 in $[$1]; do
    for word2 in [$2]; do
      if test "$word1" = "$word2"; then
        ac_var_match_word="yes"
      fi
    done
  done
])

dnl CURL_CHECK_NONBLOCKING_SOCKET
dnl -------------------------------------------------
dnl Check for how to set a socket to non-blocking state. There seems to exist
dnl four known different ways, with the one used almost everywhere being POSIX
dnl and XPG3, while the other different ways for different systems (old BSD,
dnl Windows and Amiga).
dnl
dnl There are two known platforms (AIX 3.x and SunOS 4.1.x) where the
dnl O_NONBLOCK define is found but does not work. This condition is attempted
dnl to get caught in this script by using an excessive number of #ifdefs...
dnl
AC_DEFUN([CURL_CHECK_NONBLOCKING_SOCKET],
[
  AC_MSG_CHECKING([non-blocking sockets style])

  AC_COMPILE_IFELSE([AC_LANG_PROGRAM([[
/* headers for O_NONBLOCK test */
#include <sys/types.h>
#include <unistd.h>
#include <fcntl.h>
]], [[
/* try to compile O_NONBLOCK */

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
#error "O_NONBLOCK does not work on this platform"
#endif
  int socket;
  int flags = fcntl(socket, F_SETFL, flags | O_NONBLOCK);
]])],[
dnl the O_NONBLOCK test was fine
nonblock="O_NONBLOCK"
AC_DEFINE(HAVE_O_NONBLOCK, 1, [use O_NONBLOCK for non-blocking sockets])
],[
dnl the code was bad, try a different program now, test 2

  AC_COMPILE_IFELSE([AC_LANG_PROGRAM([[
/* headers for FIONBIO test */
#include <unistd.h>
#include <stropts.h>
]], [[
/* FIONBIO source test (old-style unix) */
 int socket;
 int flags = ioctl(socket, FIONBIO, &flags);
]])],[
dnl FIONBIO test was good
nonblock="FIONBIO"
AC_DEFINE(HAVE_FIONBIO, 1, [use FIONBIO for non-blocking sockets])
],[
dnl FIONBIO test was also bad
dnl the code was bad, try a different program now, test 3

  AC_LINK_IFELSE([AC_LANG_PROGRAM([[
/* headers for IoctlSocket test (Amiga?) */
#include <sys/ioctl.h>
]], [[
/* IoctlSocket source code */
 int socket;
 int flags = IoctlSocket(socket, FIONBIO, (long)1);
]])],[
dnl ioctlsocket test was good
nonblock="IoctlSocket"
AC_DEFINE(HAVE_IOCTLSOCKET_CASE, 1, [use Ioctlsocket() for non-blocking sockets])
],[
dnl Ioctlsocket did not compile, do test 4!
  AC_COMPILE_IFELSE([AC_LANG_PROGRAM([[
/* headers for SO_NONBLOCK test (BeOS) */
#include <socket.h>
]], [[
/* SO_NONBLOCK source code */
 long b = 1;
 int socket;
 int flags = setsockopt(socket, SOL_SOCKET, SO_NONBLOCK, &b, sizeof(b));
]])],[
dnl the SO_NONBLOCK test was good
nonblock="SO_NONBLOCK"
AC_DEFINE(HAVE_SO_NONBLOCK, 1, [use SO_NONBLOCK for non-blocking sockets])
],[
dnl test 4 did not compile!
nonblock="nada"
])
dnl end of forth test

])
dnl end of third test

])
dnl end of second test

])
dnl end of non-blocking try-compile test
  AC_MSG_RESULT($nonblock)

  if test "$nonblock" = "nada"; then
    AC_MSG_WARN([non-block sockets disabled])
  fi
])

dnl CURL_CHECK_NEED_REENTRANT_SYSTEM
dnl -------------------------------------------------
dnl Checks if the preprocessor _REENTRANT definition
dnl must be unconditionally done for this platform.
dnl Internal macro for CURL_CONFIGURE_REENTRANT.

AC_DEFUN([CURL_CHECK_NEED_REENTRANT_SYSTEM], [
  case $host in
    *-*-solaris* | *-*-hpux*)
      tmp_need_reentrant="yes"
      ;;
    *)
      tmp_need_reentrant="no"
      ;;
  esac
])


dnl CURL_CONFIGURE_FROM_NOW_ON_WITH_REENTRANT
dnl -------------------------------------------------
dnl This macro ensures that configuration tests done
dnl after this will execute with preprocessor symbol
dnl _REENTRANT defined. This macro also ensures that
dnl the generated config file defines NEED_REENTRANT
dnl and that in turn setup.h will define _REENTRANT.
dnl Internal macro for CURL_CONFIGURE_REENTRANT.

AC_DEFUN([CURL_CONFIGURE_FROM_NOW_ON_WITH_REENTRANT], [
AC_DEFINE(NEED_REENTRANT, 1,
  [Define to 1 if _REENTRANT preprocessor symbol must be defined.])
cat >>confdefs.h <<_EOF
#ifndef _REENTRANT
#  define _REENTRANT
#endif
_EOF
])


dnl CURL_CONFIGURE_REENTRANT
dnl -------------------------------------------------
dnl This first checks if the preprocessor _REENTRANT
dnl symbol is already defined. If it isn't currently
dnl defined a set of checks are performed to verify
dnl if its definition is required to make visible to
dnl the compiler a set of *_r functions. Finally, if
dnl _REENTRANT is already defined or needed it takes
dnl care of making adjustments necessary to ensure
dnl that it is defined equally for further configure
dnl tests and generated config file.

AC_DEFUN([CURL_CONFIGURE_REENTRANT], [
  AC_PREREQ([2.50])dnl
  #
  AC_MSG_CHECKING([if _REENTRANT is already defined])
  AC_COMPILE_IFELSE([
    AC_LANG_PROGRAM([[
    ]],[[
#ifdef _REENTRANT
      int dummy=1;
#else
      force compilation error
#endif
    ]])
  ],[
    AC_MSG_RESULT([yes])
    tmp_reentrant_initially_defined="yes"
  ],[
    AC_MSG_RESULT([no])
    tmp_reentrant_initially_defined="no"
  ])
  #
  if test "$tmp_reentrant_initially_defined" = "no"; then
    AC_MSG_CHECKING([if _REENTRANT is actually needed])
    CURL_CHECK_NEED_REENTRANT_SYSTEM

    if test "$tmp_need_reentrant" = "yes"; then
      AC_MSG_RESULT([yes])
    else
      AC_MSG_RESULT([no])
    fi
  fi
  #
  AC_MSG_CHECKING([if _REENTRANT is onwards defined])
  if test "$tmp_reentrant_initially_defined" = "yes" ||
    test "$tmp_need_reentrant" = "yes"; then
    CURL_CONFIGURE_FROM_NOW_ON_WITH_REENTRANT
    AC_MSG_RESULT([yes])
  else
    AC_MSG_RESULT([no])
  fi
  #
])

dnl LIBSSH2_LIB_HAVE_LINKFLAGS
dnl --------------------------
dnl Wrapper around AC_LIB_HAVE_LINKFLAGS to also check $prefix/lib, if set.
dnl
dnl autoconf only checks $prefix/lib64 if gcc -print-search-dirs output
dnl includes a directory named lib64. So, to find libraries in $prefix/lib
dnl we append -L$prefix/lib to LDFLAGS before checking.
dnl
dnl For convenience, $4 is expanded if [lib]$1 is found.

AC_DEFUN([LIBSSH2_LIB_HAVE_LINKFLAGS], [
  libssh2_save_CPPFLAGS="$CPPFLAGS"
  libssh2_save_LDFLAGS="$LDFLAGS"

  if test "${with_lib$1_prefix+set}" = set; then
    CPPFLAGS="$CPPFLAGS${CPPFLAGS:+ }-I${with_lib$1_prefix}/include"
    LDFLAGS="$LDFLAGS${LDFLAGS:+ }-L${with_lib$1_prefix}/lib"
  fi

  AC_LIB_HAVE_LINKFLAGS([$1], [$2], [$3])

  if test "$ac_cv_lib$1" = "yes"; then :
    $4
  else
    CPPFLAGS="$libssh2_save_CPPFLAGS"
    LDFLAGS="$libssh2_save_LDFLAGS"
  fi
])

AC_DEFUN([LIBSSH2_CHECK_CRYPTO], [
if test "$use_crypto" = "auto" && test "$found_crypto" = "none" || test "$use_crypto" = "$1"; then
m4_case([$1],
[openssl], [
  LIBSSH2_LIB_HAVE_LINKFLAGS([ssl], [crypto], [#include <openssl/ssl.h>], [
    AC_DEFINE(LIBSSH2_OPENSSL, 1, [Use $1])
    LIBSSH2_PC_REQUIRES_PRIVATE="$LIBSSH2_PC_REQUIRES_PRIVATE${LIBSSH2_PC_REQUIRES_PRIVATE:+,}libcrypto"
    found_crypto="$1"
    found_crypto_str="OpenSSL"
  ])
],

[wolfssl], [
  LIBSSH2_LIB_HAVE_LINKFLAGS([wolfssl], [], [#include <wolfssl/options.h>], [
    AC_DEFINE(LIBSSH2_WOLFSSL, 1, [Use $1])
    LIBSSH2_PC_REQUIRES_PRIVATE="$LIBSSH2_PC_REQUIRES_PRIVATE${LIBSSH2_PC_REQUIRES_PRIVATE:+,}wolfssl"
    found_crypto="$1"
  ])
],

[libgcrypt], [
  LIBSSH2_LIB_HAVE_LINKFLAGS([gcrypt], [], [#include <gcrypt.h>], [
    AC_DEFINE(LIBSSH2_LIBGCRYPT, 1, [Use $1])
    LIBSSH2_PC_REQUIRES_PRIVATE="$LIBSSH2_PC_REQUIRES_PRIVATE${LIBSSH2_PC_REQUIRES_PRIVATE:+,}libgcrypt"
    found_crypto="$1"
  ])
],

[mbedtls], [
  LIBSSH2_LIB_HAVE_LINKFLAGS([mbedcrypto], [], [#include <mbedtls/version.h>], [
    AC_DEFINE(LIBSSH2_MBEDTLS, 1, [Use $1])
    LIBS="$LIBS -lmbedcrypto"
    found_crypto="$1"
  ])
],

[wincng], [
  if test "x$have_windows_h" = "xyes"; then
    # Look for Windows Cryptography API: Next Generation

    LIBS="$LIBS -lcrypt32"

    # Check necessary for old-MinGW
    LIBSSH2_LIB_HAVE_LINKFLAGS([bcrypt], [], [
      #include <windows.h>
      #include <bcrypt.h>
    ], [
      AC_DEFINE(LIBSSH2_WINCNG, 1, [Use $1])
      found_crypto="$1"
      found_crypto_str="Windows Cryptography API: Next Generation"
    ])
  fi
],
)
  test "$found_crypto" = "none" &&
    crypto_errors="${crypto_errors}No $1 crypto library found!
"
fi
])


dnl LIBSSH2_CHECK_OPTION_WERROR
dnl -------------------------------------------------
dnl Verify if configure has been invoked with option
dnl --enable-werror or --disable-werror, and set
dnl shell variable want_werror as appropriate.

AC_DEFUN([LIBSSH2_CHECK_OPTION_WERROR], [
  AC_BEFORE([$0],[LIBSSH2_CHECK_COMPILER])dnl
  AC_MSG_CHECKING([whether to enable compiler warnings as errors])
  OPT_COMPILER_WERROR="default"
  AC_ARG_ENABLE(werror,
AS_HELP_STRING([--enable-werror],[Enable compiler warnings as errors])
AS_HELP_STRING([--disable-werror],[Disable compiler warnings as errors]),
  OPT_COMPILER_WERROR=$enableval)
  case "$OPT_COMPILER_WERROR" in
    no)
      dnl --disable-werror option used
      want_werror="no"
      ;;
    default)
      dnl configure option not specified
      want_werror="no"
      ;;
    *)
      dnl --enable-werror option used
      want_werror="yes"
      ;;
  esac
  AC_MSG_RESULT([$want_werror])

  if test X"$want_werror" = Xyes; then
    LIBSSH2_CFLAG_EXTRAS="$LIBSSH2_CFLAG_EXTRAS -Werror"
  fi
])
