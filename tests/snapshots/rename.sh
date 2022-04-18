#!/usr/bin/env bash
BEFORE=$1
AFTER=$2
if [ -z "${BEFORE}" ] || [ -z "${AFTER}" ]; then
    echo "Rename a snapshot"
    echo
    echo "Run this from the snapshots directory, using the base name for the snapshots"
    echo
    echo "Usage: $0 <BEFORE> <AFTER>"
    exit 1
fi

if [ -e "$1.stdout" ]; then
    echo "Renaming '$1.in"
    mv $1.in $2.in
else
    echo "No '$1.in'"
fi
if [ -e "$1.stdout" ]; then
    echo "Renaming '$1.out"
    mv $1.out $2.out
else
    echo "No '$1.out'"
fi
if [ -e "$1.stdout" ]; then
    echo "Renaming '$1.stdout"
    mv $1.stdout $2.stdout
else
    echo "No '$1.stdout'"
fi
if [ -e "$1.stderr" ]; then
    echo "Renaming '$1.stderr'"
    mv $1.stderr $2.stderr
else
    echo "No '$1.stderr'"
fi
git add $2.*
