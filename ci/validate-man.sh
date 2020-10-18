#!/bin/bash
# This script validates that there aren't any changes to the man pages.

set -e

cd src/doc

changes=$(git status --porcelain)
if [ -n "$changes" ]
then
    echo "git directory must be clean before running this script."
    exit 1
fi

./build-man.sh

changes=$(git status --porcelain)
if [ -n "$changes" ]
then
    echo "Detected changes in man pages:"
    echo "$changes"
    echo
    echo "Please run './build-man.sh' in the src/doc directory to rebuild the"
    echo "man pages, and commit the changes."
    exit 1
fi
