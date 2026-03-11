#!/bin/bash
# This script validates that there aren't any changes to the man pages.

set -e

cargo_man="src/doc"
mdman_man="crates/mdman/doc"
man_out="src/etc/man"

changes=$(git status --porcelain -- $cargo_man $mdman_man $man_out)
if [ -n "$changes" ]
then
    echo "git directory must be clean before running this script."
    exit 1
fi

cargo build-man

changes=$(git status --porcelain -- $cargo_man $mdman_man $man_out)
if [ -n "$changes" ]
then
    echo "Detected changes of man pages:"
    echo "$changes"
    echo
    echo 'Please run `cargo build-man` to rebuild the man pages'
    echo "and commit the changes."
    exit 1
fi
