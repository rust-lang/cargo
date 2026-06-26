#!/bin/sh
# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
#
#       libssh2 compilation script for the OS/400.
#

SCRIPTDIR=$(dirname "${0}")
. "${SCRIPTDIR}/initscript.sh"
cd "${TOPDIR}/src" || exit 1


#       Function to extract external prototypes from header files.
#       Input: concatenated header files.
#       Output: external prototypes, one per (long) line.

extproto()
{
        sed -e 'x;G;s/^\n//;s/\n/ /g'                                   \
            -e 's#[[:space:]]*/\*[^*]*\(\*\([^/*][^*]*\)\{0,1\}\)*\*/[[:space:]]*##g' \
            -e 'h'                                                      \
            -e '/\/\*/!{'                                               \
            -e '/^#/{s/^.*[^\\]$//;h;d'                                 \
            -e '}'                                                      \
            -e 's/[{}]/;/g;s/\\$//'                                     \
            -e ':loop1'                                                 \
            -e '/;/{'                                                   \
            -e 's/^[^;]*;//;x;s/;.*//'                                  \
            -e '/^[[:space:]]*LIBSSH2_API[[:space:]].*(/{'              \
            -e 's/^[[:space:]]*LIBSSH2_API[[:space:]]*//'               \
            -e 's/[[:space:]]*$//'                                      \
            -e 's/[[:space:]][[:space:]]*/ /g'                          \
            -e 'p'                                                      \
            -e '}'                                                      \
            -e 'g;bloop1'                                               \
            -e '}'                                                      \
            -e '}'                                                      \
            -n
}

#       Need to have IFS access to the mih/modasa header file.

if action_needed modasa.mih '/QSYS.LIB/QSYSINC.LIB/MIH.FILE/MODASA.MBR'
then    rm -f modasa.mih
        ln -s '/QSYS.LIB/QSYSINC.LIB/MIH.FILE/MODASA.MBR' modasa.mih
fi


#      Create and compile the identification source file.

{
        echo '#pragma comment(user, "libssh2 version '"${LIBSSH2_VERSION}"'")'
        echo '#pragma comment(user, __DATE__)'
        echo '#pragma comment(user, __TIME__)'
        echo '#pragma comment(copyright, "See COPYING file. OS/400 version by P. Monnerat")'
} > os400.c
make_module     OS400           os400.c
LINK=                           # No need to rebuild service program yet.
MODULES=


#       Generate the procedures implementing macros.

if action_needed macros.c "${TOPDIR}/os400/macros.h"
then    (
                echo '#include "libssh2_publickey.h"'
                echo '#include "libssh2_sftp.h"'
                extproto < "${TOPDIR}/os400/macros.h"                   |
                sed -e 'h;s/^[^(]*[ *]\([^ (]*\) *(.*/\1/'              \
                    -e 's/.*/#pragma map(_&, "&")/;p'                   \
                    -e 'g;s/^\([^(]*[ *]\)\([^ (]*\)\( *(.*\)/\1_\2\3 {/;p' \
                    -e 'g;s/^[^(]*(\(.*\))$/,\1,/;s/[^A-Za-z0-9_,]/ /g' \
                    -e 's/  *,/,/g;s/,[^,]* \([^ ,]*\)/,\1/g'           \
                    -e 's/ //g;s/^,void,$/,,/'                          \
                    -e 's/^,\(.*\),$/(\1); }/;s/,/, /g'                 \
                    -e 'x;s/(.*//;s/ *$//;G;s/\n//g'                    \
                    -e 's/^void\([ *]\)/\1/;s/^ *//'                    \
                    -e 's/^[^(]*[ *]\([A-Za-z][A-Za-z0-9_]* *(\)/return \1/' \
                    -e 's/.*/    &/'
        ) > macros.c
fi

#       Get source list.

sed -e ':begin'                                                         \
  -e '/\\$/{'                                                           \
  -e 's/\\$/ /'                                                         \
  -e 'N'                                                                \
  -e 'bbegin'                                                           \
  -e '}'                                                                \
  -e 's/\n//g'                                                          \
  -e 's/[[:space:]]*$//'                                                \
  -e 's/^\([A-Za-z][A-Za-z0-9_]*\)[[:space:]]*=[[:space:]]*\(.*\)/\1="\2"/' \
  -e 's/\$(\([A-Za-z][A-Za-z0-9_]*\))/${\1}/g'                          \
      < Makefile.inc > tmpscript.sh
. ./tmpscript.sh


#       Compile the sources into modules.

# shellcheck disable=SC2034
INCLUDES="'$(pwd)'"

for SRC in "${TOPDIR}/os400/os400sys.c" "${TOPDIR}/os400/ccsid.c"       \
           ${CSOURCES} macros.c
do      MODULE=$(db2_name "${SRC}")
        make_module "${MODULE}" "${SRC}"
done


#       If needed, (re)create the static binding directory.

if action_needed "${LIBIFSNAME}/${STATBNDDIR}.BNDDIR"
then    LINK=YES
fi

if [ -n "${LINK}" ]
then    rm -rf "${LIBIFSNAME}/${STATBNDDIR}.BNDDIR"
        CMD="CRTBNDDIR BNDDIR(${TARGETLIB}/${STATBNDDIR})"
        CMD="${CMD} TEXT('libssh2 API static binding directory')"
        system "${CMD}"

        for MODULE in ${MODULES}
        do      CMD="ADDBNDDIRE BNDDIR(${TARGETLIB}/${STATBNDDIR})"
                CMD="${CMD} OBJ((${TARGETLIB}/${MODULE} *MODULE))"
                system "${CMD}"
        done

#       V6R1M0 does not list system service program QC3PBEXT in the
#       implicit binding directory: thus we add it here in ours.

        CMD="ADDBNDDIRE BNDDIR(${TARGETLIB}/${STATBNDDIR})"
        CMD="${CMD} OBJ((QSYS/QC3PBEXT *SRVPGM))"
        system "${CMD}"
fi


#       The exportation file for service program creation must be in a DB2
#               source file, so make sure it exists.

if action_needed "${LIBIFSNAME}/TOOLS.FILE"
then    CMD="CRTSRCPF FILE(${TARGETLIB}/TOOLS) RCDLEN(112)"
        CMD="${CMD} TEXT('libssh2: build tools')"
        system "${CMD}"
fi


#       Gather the list of symbols to export.

EXPORTS=$(cat "${TOPDIR}"/include/*.h "${TOPDIR}/os400/macros.h"        \
             "${TOPDIR}/os400/libssh2_ccsid.h"                          |
         extproto                                                       |
         sed -e 's/(.*//;s/[^A-Za-z0-9_]/ /g;s/ *$//;s/^.* //')

#       Create the service program exportation file in DB2 member if needed.

BSF="${LIBIFSNAME}/TOOLS.FILE/BNDSRC.MBR"

if action_needed "${BSF}" Makefile.am
then    LINK=YES
fi

if [ -n "${LINK}" ]
then    echo " STRPGMEXP PGMLVL(*CURRENT) SIGNATURE('LIBSSH2_${SONAME}')" \
            > "${BSF}"
        for EXPORT in ${EXPORTS}
        do      echo ' EXPORT    SYMBOL("'"${EXPORT}"'")' >> "${BSF}"
        done

        echo ' ENDPGMEXP' >> "${BSF}"
fi


#       Build the service program if needed.

if action_needed "${LIBIFSNAME}/${SRVPGM}.SRVPGM"
then    LINK=YES
fi

if [ -n "${LINK}" ]
then    CMD="CRTSRVPGM SRVPGM(${TARGETLIB}/${SRVPGM})"
        CMD="${CMD} SRCFILE(${TARGETLIB}/TOOLS) SRCMBR(BNDSRC)"
        CMD="${CMD} MODULE(${TARGETLIB}/OS400)"
        CMD="${CMD} BNDDIR(${TARGETLIB}/${STATBNDDIR}"
        if [ "${WITH_ZLIB}" != 0 ]
        then    CMD="${CMD} ${ZLIB_LIB}/${ZLIB_BNDDIR}"
                liblist -a "${ZLIB_LIB}"
        fi
        CMD="${CMD})"
        CMD="${CMD} BNDSRVPGM(QADRTTS)"
        CMD="${CMD} TEXT('libssh2 API library')"
        CMD="${CMD} TGTRLS(${TGTRLS})"
        system "${CMD}"
        LINK=YES
fi


#       If needed, (re)create the dynamic binding directory.

if action_needed "${LIBIFSNAME}/${DYNBNDDIR}.BNDDIR"
then    LINK=YES
fi

if [ -n "${LINK}" ]
then    rm -rf "${LIBIFSNAME}/${DYNBNDDIR}.BNDDIR"
        CMD="CRTBNDDIR BNDDIR(${TARGETLIB}/${DYNBNDDIR})"
        CMD="${CMD} TEXT('libssh2 API dynamic binding directory')"
        system "${CMD}"
        CMD="ADDBNDDIRE BNDDIR(${TARGETLIB}/${DYNBNDDIR})"
        CMD="${CMD} OBJ((*LIBL/${SRVPGM} *SRVPGM))"
        system "${CMD}"
fi
