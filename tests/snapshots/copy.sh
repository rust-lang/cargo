#!/usr/bin/env bash
BEFORE=$1
AFTER=$2
if [ -z "${BEFORE}" ] || [ -z "${AFTER}" ]; then
    echo "Copy a snapshot"
    echo
    echo "Run this from the snapshots directory, using the base name for the snapshots"
    echo
    echo "Usage: $0 <BEFORE> <AFTER>"
    exit 1
fi

IN_LINK=$(readlink $1.in)
if [ -n "${IN_LINK}" ]; then
    echo "Linking to '${IN_LINK}'"
    ln -s ${IN_LINK} $2.in
elif [ -e "$1.in" ]; then
    echo "Copying '$1.in'"
    cp -r $1.in $2.in
else
    echo "No '$1.in'"
fi
if [ -e "$1.out" ]; then
    echo "Copying '$1.out'"
    cp -r $1.out $2.out
else
    echo "No '$1.out'"
fi
if [ -e "$1.stdout" ]; then
    echo "Copying '$1.stdout'"
    cp -r $1.stdout $2.stdout
else
    echo "No '$1.stdout'"
fi
if [ -e "$1.stderr" ]; then
    echo "Copying '$1.stderr'"
    cp -r $1.stderr $2.stderr
else
    echo "No '$1.stderr'"
fi
git add $2.*
