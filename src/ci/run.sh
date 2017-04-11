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
    BRANCH=$APPVEYOR_REPO_BRANCH
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

# We want to only run tests in a few situations:
#
# * All tests on the auto-cargo branch
# * One test on PRs
# * Any tests done locally
#
# This means that here if we're on CI, then we skip tests if it's not the right
# branch or if we're not configured to run a test on PRs
if [ -n "$CI" ] && [ "$BRANCH" != "auto-cargo" ] && [ "$ALLOW_PR" = "" ]; then
    echo no build necessary, skipping
    exit 0
fi

# For some unknown reason libz is not found in the android docker image, so we
# use this workaround
if [ "$TARGET" = armv7-linux-androideabi ]; then
    export DEP_Z_ROOT=/android-ndk/arm/sysroot/usr
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
