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

set -e

script=`cd $(dirname $0) && pwd`/`basename $0`
image=$1
TARGET=$2

docker_dir="`dirname $script`"
ci_dir="`dirname $docker_dir`"
src_dir="`dirname $ci_dir`"
root_dir="`dirname $src_dir`"

docker build \
  --rm \
  -t rust-ci \
  "`dirname "$script"`/$image"

mkdir -p $HOME/.cargo
mkdir -p target

exec docker run \
  --user `id -u`:`id -g` \
  --volume "$root_dir:/checkout:ro" \
  --workdir /tmp \
  --env CFG_DISABLE_CROSS_TESTS=$CFG_DISABLE_CROSS_TESTS \
  --env MAKE_TARGETS="$MAKE_TARGETS" \
  --env SRC=/checkout \
  --env CARGO_HOME=/cargo \
  --volume "$HOME/.cargo:/cargo" \
  --volume `rustc --print sysroot`:/rust:ro \
  --volume `pwd`/target:/tmp/target \
  --interactive \
  --tty \
  rust-ci \
  sh -c "\
    PATH=\$PATH:/rust/bin \
    LD_LIBRARY_PATH=/rust/lib:\$LD_LIBRARY_PATH \
    /checkout/src/ci/run.sh $TARGET"

