#!/bin/sh
# Copyright 2016 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

set -ex

TARGET=$1

if [ -z "$SRC" ]; then
    SRC=.
fi

BRANCH=$TRAVIS_BRANCH
if [ "$BRANCH" = "" ]; then
    BRANCH=$APPVEYOR_BRANCH
fi

if [ "$BRANCH" = "stable" ]; then
    CHANNEL=stable
elif [ "$BRANCH" = "beta" ]; then
    CHANNEL=beta
elif [ "$BRANCH" = "master" ]; then
    CHANNEL=nightly
elif [ "$BRANCH" = "auto-cargo" ]; then
    CHANNEL=nightly
else
    CHANNEL=dev
fi

$SRC/configure \
    --prefix=/tmp/obj/install \
    --target=$TARGET \
    --release-channel=$CHANNEL \
    --enable-build-openssl

make cargo-$TARGET
make dist-$TARGET

if [ ! -z "$MAKE_TARGETS" ]; then
  for target in "$MAKE_TARGETS"; do
    make $target
  done
fi
