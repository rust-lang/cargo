#!/bin/sh
# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
#
#       libssh2 compilation script for the OS/400.
#
#
#       This is a shell script since make is not a standard component of OS/400.

SCRIPTDIR=$(dirname "${0}")
. "${SCRIPTDIR}/initscript.sh"
cd "${TOPDIR}" || exit 1


#       Create the OS/400 library if it does not exist.

if action_needed "${LIBIFSNAME}"
then    CMD="CRTLIB LIB(${TARGETLIB}) TEXT('libssh2: SSH2 protocol API')"
        system "${CMD}"
fi


#       Create the DOCS source file if it does not exist.

if action_needed "${LIBIFSNAME}/DOCS.FILE"
then    CMD="CRTSRCPF FILE(${TARGETLIB}/DOCS) RCDLEN(240)"
        CMD="${CMD} CCSID(${TGTCCSID}) TEXT('Documentation texts')"
        system "${CMD}"
fi


#       Copy some documentation files if needed.

for TEXT in "${TOPDIR}/COPYING" "${SCRIPTDIR}/README400"                \
    "${TOPDIR}/NEWS" "${TOPDIR}/README" "${TOPDIR}/docs/AUTHORS"        \
    "${TOPDIR}/docs/BINDINGS.md"
do      MEMBER="${LIBIFSNAME}/DOCS.FILE/$(db2_name "${TEXT}").MBR"

        if action_needed "${MEMBER}" "${TEXT}"
        then    CMD="CPY OBJ('${TEXT}') TOOBJ('${MEMBER}') TOCCSID(${TGTCCSID})"
                CMD="${CMD} DTAFMT(*TEXT) REPLACE(*YES)"
                system "${CMD}"
        fi
done


#       Create the RPGXAMPLES source file if it does not exist.

if action_needed "${LIBIFSNAME}/RPGXAMPLES.FILE"
then    CMD="CRTSRCPF FILE(${TARGETLIB}/RPGXAMPLES) RCDLEN(240)"
        CMD="${CMD} CCSID(${TGTCCSID}) TEXT('ILE/RPG examples')"
        system "${CMD}"
fi


#       Copy RPG examples if needed.

for EXAMPLE in "${SCRIPTDIR}/rpg-examples"/*
do      MEMBER="$(basename "${EXAMPLE}")"
        IFSMEMBER="${LIBIFSNAME}/RPGXAMPLES.FILE/$(db2_name "${MEMBER}").MBR"

        [ -e "${EXAMPLE}" ] || continue

        if action_needed "${IFSMEMBER}" "${EXAMPLE}"
        then    CMD="CPY OBJ('${EXAMPLE}') TOOBJ('${IFSMEMBER}')"
                CMD="${CMD} TOCCSID(${TGTCCSID}) DTAFMT(*TEXT) REPLACE(*YES)"
                system "${CMD}"
                MBRTEXT=$(sed -e '1!d;/^      \*/!d;s/^ *\* *//'        \
                              -e 's/ *$//;s/'"'"'/&&/g' < "${EXAMPLE}")
                CMD="CHGPFM FILE(${TARGETLIB}/RPGXAMPLES) MBR(${MEMBER})"
                CMD="${CMD} SRCTYPE(RPGLE) TEXT('${MBRTEXT}')"
                system "${CMD}"
        fi
done


#       Build in each directory.

for SUBDIR in include rpg src
do      "${SCRIPTDIR}/make-${SUBDIR}.sh"
done
