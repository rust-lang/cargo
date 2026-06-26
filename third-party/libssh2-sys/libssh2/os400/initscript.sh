#!/bin/sh
# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause

setenv()

{
        #       Define and export.

        eval "${1}=${2}"
        export "${1?}"
}


case "${SCRIPTDIR}" in
/*)     ;;
*)      SCRIPTDIR="$(pwd)/${SCRIPTDIR}"
esac

while true
do      case "${SCRIPTDIR}" in
        */.)    SCRIPTDIR="${SCRIPTDIR%/.}";;
        *)      break;;
        esac
done

#  The script directory is supposed to be in $TOPDIR/os400.

TOPDIR=$(dirname "${SCRIPTDIR}")
export SCRIPTDIR TOPDIR

#  Extract the SONAME from the library makefile.

SONAME=$(sed -e '/^VERSION=/!d'                                         \
             -e 's/^.* \([0-9]*\):.*$/\1/' -e 'q'                       \
                                                < "${TOPDIR}/src/Makefile.am")
export SONAME

#       Get OS/400 configuration parameters.

. "${SCRIPTDIR}/config400.default"
if [ -f "${SCRIPTDIR}/config400.override" ]
then    . "${SCRIPTDIR}/config400.override"
fi

#       Need to get the version definitions.

LIBSSH2_VERSION=$(grep '^#define  *LIBSSH2_VERSION '                    \
                        "${TOPDIR}/include/libssh2.h"                   |
                sed 's/.*"\(.*\)".*/\1/')
LIBSSH2_VERSION_MAJOR=$(grep '^#define  *LIBSSH2_VERSION_MAJOR '        \
                        "${TOPDIR}/include/libssh2.h"                   |
                sed 's/^#define  *LIBSSH2_VERSION_MAJOR  *\([^ ]*\).*/\1/')
LIBSSH2_VERSION_MINOR=$(grep '^#define  *LIBSSH2_VERSION_MINOR '        \
                        "${TOPDIR}/include/libssh2.h"                   |
                sed 's/^#define  *LIBSSH2_VERSION_MINOR  *\([^ ]*\).*/\1/')
LIBSSH2_VERSION_PATCH=$(grep '^#define  *LIBSSH2_VERSION_PATCH '        \
                        "${TOPDIR}/include/libssh2.h"                   |
                sed 's/^#define  *LIBSSH2_VERSION_PATCH  *\([^ ]*\).*/\1/')
LIBSSH2_VERSION_NUM=$(grep '^#define  *LIBSSH2_VERSION_NUM '            \
                        "${TOPDIR}/include/libssh2.h"                   |
                sed 's/^#define  *LIBSSH2_VERSION_NUM  *0x\([^ ]*\).*/\1/')
LIBSSH2_TIMESTAMP=$(grep '^#define  *LIBSSH2_TIMESTAMP '                \
                        "${TOPDIR}/include/libssh2.h"                   |
                sed 's/.*"\(.*\)".*/\1/')
export LIBSSH2_VERSION
export LIBSSH2_VERSION_MAJOR LIBSSH2_VERSION_MINOR LIBSSH2_VERSION_PATCH
export LIBSSH2_VERSION_NUM LIBSSH2_TIMESTAMP

################################################################################
#
#                       OS/400 specific definitions.
#
################################################################################

LIBIFSNAME="/QSYS.LIB/${TARGETLIB}.LIB"


################################################################################
#
#                               Procedures.
#
################################################################################

#       action_needed dest [src]
#
#       dest is an object to build
#       if specified, src is an object on which dest depends.
#
#       exit 0 (succeeds) if some action has to be taken, else 1.

action_needed()

{
        [ ! -e "${1}" ] && return 0
        [ -n "${2}" ] || return 1
        # shellcheck disable=SC3013
        [ "${1}" -ot "${2}" ] && return 0
        return 1
}


#       canonicalize_path path
#
#       Return canonicalized path as:
#       - Absolute
#       - No . or .. component.

canonicalize_path()

{
        if expr "${1}" : '^/' > /dev/null
        then    P="${1}"
        else    P="$(pwd)/${1}"
        fi

        R=
        IFSSAVE="${IFS}"
        IFS="/"

        for C in ${P}
        do      IFS="${IFSSAVE}"
                case "${C}" in
                .)      ;;
                ..)     R="$(expr "${R}" : '^\(.*/\)..*')"
                        ;;
                ?*)     R="${R}${C}/"
                        ;;
                *)      ;;
                esac
        done

        IFS="${IFSSAVE}"
        echo "/$(expr "${R}" : '^\(.*\)/')"
}


#       make_module module_name source_name [additional_definitions]
#
#       Compile source name into ASCII module if needed.
#       As side effect, append the module name to variable MODULES.
#       Set LINK to "YES" if the module has been compiled.

make_module()

{
        MODULES="${MODULES} ${1}"
        MODIFSNAME="${LIBIFSNAME}/${1}.MODULE"
        action_needed "${MODIFSNAME}" "${2}" || return 0;
        SRCDIR="$(dirname "$(canonicalize_path "${2}")")"

        #       #pragma convert has to be in the source file itself, i.e.
        #               putting it in an include file makes it only active
        #               for that include file.
        #       Thus we build a temporary file with the pragma prepended to
        #               the source file and we compile that temporary file.

        {
                echo "#line 1 \"${2}\""
                echo "#pragma convert(819)"
                echo "#line 1"
                cat "${2}"
        } > __tmpsrcf.c
        CMD="CRTCMOD MODULE(${TARGETLIB}/${1}) SRCSTMF('__tmpsrcf.c')"
#       CMD="${CMD} SYSIFCOPT(*IFS64IO) OPTION(*INCDIRFIRST *SHOWINC *SHOWSYS)"
        CMD="${CMD} SYSIFCOPT(*IFS64IO) OPTION(*INCDIRFIRST)"
        CMD="${CMD} LOCALETYPE(*LOCALE) FLAG(10)"
        CMD="${CMD} INCDIR('${TOPDIR}/os400/include'"
        CMD="${CMD} '${QADRTDIR}/include' '${TOPDIR}/include'"
        CMD="${CMD} '${TOPDIR}/os400' '${SRCDIR}'"

        if [ "${WITH_ZLIB}" != "0" ]
        then    CMD="${CMD} '${ZLIB_INCLUDE}'"
        fi

        CMD="${CMD} ${INCLUDES})"
        CMD="${CMD} TGTCCSID(${TGTCCSID}) TGTRLS(${TGTRLS})"
        CMD="${CMD} OUTPUT(${OUTPUT})"
        CMD="${CMD} OPTIMIZE(${OPTIMIZE})"
        CMD="${CMD} DBGVIEW(${DEBUG})"

        DEFINES="${3}"

        if [ "${WITH_ZLIB}" != "0" ]
        then    DEFINES="${DEFINES} LIBSSH2_HAVE_ZLIB"
        fi

        if [ "${WITH_MD5}" != 'yes' ]
        then    DEFINES="${DEFINES} LIBSSH2_NO_MD5"
        fi

        if [ -n "${DEFINES}" ]
        then    CMD="${CMD} DEFINE(${DEFINES})"
        fi

        system "${CMD}"
        rm -f __tmpsrcf.c
        # shellcheck disable=SC2034
        LINK=YES
}


#       Determine DB2 object name from IFS name.

db2_name()

{
        if [ "${2}" = 'nomangle' ]
        then    basename "${1}"                                         |
                tr 'a-z-' 'A-Z_'                                        |
                sed -e 's/\..*//;s/^\(.\).*\(.........\)$/\1\2/'
        else    basename "${1}"                                         |
                tr 'a-z-' 'A-Z_'                                        |
                sed -e 's/\..*//;s/^LIBSSH2_/SSH2_/'                    \
                    -e 's/^\(.\).*\(.........\)$/\1\2/'                 \
                    -e 's/^SPUBLICKEY$/SSH2_PKEY/'
        fi
}


#       Copy stream replacing version info.

versioned_copy()

{
        sed -e "s/@LIBSSH2_VERSION@/${LIBSSH2_VERSION}/g"               \
            -e "s/@LIBSSH2_VERSION_MAJOR@/${LIBSSH2_VERSION_MAJOR}/g"   \
            -e "s/@LIBSSH2_VERSION_MINOR@/${LIBSSH2_VERSION_MINOR}/g"   \
            -e "s/@LIBSSH2_VERSION_PATCH@/${LIBSSH2_VERSION_PATCH}/g"   \
            -e "s/@LIBSSH2_VERSION_NUM@/${LIBSSH2_VERSION_NUM}/g"       \
            -e "s/@LIBSSH2_TIMESTAMP@/${LIBSSH2_TIMESTAMP}/g"
}
