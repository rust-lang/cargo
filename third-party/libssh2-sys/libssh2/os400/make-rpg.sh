#!/bin/sh
# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
#
#       Installation of the ILE/RPG header files in the OS/400 library.
#

SCRIPTDIR=$(dirname "${0}")
. "${SCRIPTDIR}/initscript.sh"
cd "${TOPDIR}/os400/libssh2rpg" || exit 1


#       Create the OS/400 source program file for the ILE/RPG header files.

SRCPF="${LIBIFSNAME}/LIBSSH2RPG.FILE"

if action_needed "${SRCPF}"
then    CMD="CRTSRCPF FILE(${TARGETLIB}/LIBSSH2RPG) RCDLEN(112)"
        CMD="${CMD} CCSID(${TGTCCSID}) TEXT('libssh2: ILE/RPG header files')"
        system "${CMD}"
fi


#       Map file names to DB2 name syntax.

for HFILE in *.rpgle *.rpgle.in
do      NAME="$(basename "${HFILE}" .in)"
        VAR="$(basename "${NAME}" .rpgle)"
        VAL="$(db2_name "${NAME}")"

        eval "VAR_${VAR}=\"${VAL}\""
        echo "${VAR} s/${VAR}/${VAL}/g"
done > tmpsubstfile1

#       Order substitution commands so that a prefix appears after all
#               file names beginning with the prefix.

sort -r tmpsubstfile1 | sed 's/^[^ ]*[ ]*//' > tmpsubstfile2


change_include()

{
        sed -e '\#^....../include  *"libssh2rpg/#{'                     \
            -e 's///'                                                   \
            -e 's/".*//'                                                \
            -f tmpsubstfile2                                            \
            -e 's#.*#      /include libssh2rpg,&#'                      \
            -e '}'
}


#       Create the IFS directory for the ILE/RPG header files.

RPGIFSDIR="${IFSDIR}/include/libssh2rpg"

if action_needed "${RPGIFSDIR}"
then    mkdir -p "${RPGIFSDIR}"
fi

#       Copy the header files to IFS ILE/RPG include directory.
#       Copy them with include path editing to the DB2 library.

for HFILE in *.rpgle *.rpgle.in
do      IFSCMD="cat \"${HFILE}\""
        DB2CMD="change_include < \"${HFILE}\""
        IFSFILE="$(basename "${HFILE}" .in)"

        case "${HFILE}" in

        *.in)   IFSCMD="${IFSCMD} | versioned_copy"
                DB2CMD="${DB2CMD} | versioned_copy"
                ;;
        esac

        IFSDEST="${RPGIFSDIR}/${IFSFILE}"

        if action_needed "${IFSDEST}" "${HFILE}"
        then    eval "${IFSCMD}" > "${IFSDEST}"
        fi

        eval DB2MBR="\"\${VAR_$(basename "${IFSDEST}" .rpgle)}\""
        DB2DEST="${SRCPF}/${DB2MBR}.MBR"

        if action_needed "${DB2DEST}" "${HFILE}"
        then    eval "${DB2CMD}" | change_include > tmphdrfile

                #       Need to translate to target CCSID.

                CMD="CPY OBJ('$(pwd)/tmphdrfile') TOOBJ('${DB2DEST}')"
                CMD="${CMD} TOCCSID(${TGTCCSID}) DTAFMT(*TEXT) REPLACE(*YES)"
                system "${CMD}"
        fi
done
