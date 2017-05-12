#!/bin/bash

root_dir=$(pwd)

cd tests/fixtures/libui-rs/ui
for d in $root_dir/tests/fixtures/libui-rs.tests/*/
do
    echo $d
    git checkout .
    cargo clean
    cat $d/input.txt | cargo run --manifest-path=$root_dir/Cargo.toml -- --clippy > output.txt
    if test "$1" == "apply"
    then
        git diff > $d/diff.diff
        cp output.txt $d/output.txt
    else
        git diff > diff.diff
        set -e
        diff diff.diff $d/diff.diff
        diff output.txt $d/output.txt
        set +e
    fi
done
