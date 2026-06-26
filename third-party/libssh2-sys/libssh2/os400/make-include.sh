#!/bin/sh
# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
#
#       Installation of the header files in the OS/400 library.
#

SCRIPTDIR=$(dirname "${0}")
. "${SCRIPTDIR}/initscript.sh"
cd "${TOPDIR}/include" || exit 1


#       Create the OS/400 source program file for the header files.

SRCPF="${LIBIFSNAME}/H.FILE"

if action_needed "${SRCPF}"
then    CMD="CRTSRCPF FILE(${TARGETLIB}/H) RCDLEN(112)"
        CMD="${CMD} CCSID(${TGTCCSID}) TEXT('libssh2: Header files')"
        system "${CMD}"
fi


#       Create the IFS directory for the header files.

IFSINCLUDE="${IFSDIR}/include"

if action_needed "${IFSINCLUDE}"
then    mkdir -p "${IFSINCLUDE}"
fi


copy_hfile()

{
        destfile="${1}"
        srcfile="${2}"
        shift
        shift
        sed -e '1i\
#pragma datamodel(P128)\
' "${@}" -e '$a\
#pragma datamodel(pop)\
' < "${srcfile}" > "${destfile}"
}

#       Copy the header files.

for HFILE in *.h "${TOPDIR}/os400/libssh2_ccsid.h"
do      DEST="${SRCPF}/$(db2_name "${HFILE}").MBR"

        if action_needed "${DEST}" "${HFILE}"
        then    copy_hfile "${DEST}" "${HFILE}"
                IFSDEST="${IFSINCLUDE}/$(basename "${HFILE}")"
                rm -f "${IFSDEST}"
                ln -s "${DEST}" "${IFSDEST}"
        fi
done
