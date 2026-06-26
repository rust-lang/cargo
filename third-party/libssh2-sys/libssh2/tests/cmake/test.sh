#!/bin/sh
#
# Copyright (C) Viktor Szakats
# SPDX-License-Identifier: BSD-3-Clause

set -e
set -u

cd "$(dirname "$0")"

rm -rf bld-fetchcontent; cmake -B bld-fetchcontent -DTEST_INTEGRATION_MODE=FetchContent \
  -DFROM_GIT_REPO="${PWD}/../.." \
  -DFROM_GIT_TAG="$(git rev-parse HEAD)"
make -j3 -C bld-fetchcontent

rm -rf libssh2; ln -s ../.. libssh2
rm -rf bld-add_subdirectory; cmake -B bld-add_subdirectory -DTEST_INTEGRATION_MODE=add_subdirectory
make -j3 -C bld-add_subdirectory

rm -rf bld-libssh2; cmake ../.. -B bld-libssh2
make -j3 -C bld-libssh2 DESTDIR=pkg install
rm -rf bld-find_package; cmake -B bld-find_package -DTEST_INTEGRATION_MODE=find_package \
  -DCMAKE_PREFIX_PATH="${PWD}/bld-libssh2/pkg/usr/local/lib/cmake/libssh2"
make -j3 -C bld-find_package

(cd ../..; git archive --format=tar HEAD) | gzip > source.tar.gz
rm -rf bld-externalproject; cmake -B bld-externalproject -DTEST_INTEGRATION_MODE=ExternalProject \
  -DFROM_ARCHIVE="${PWD}/source.tar.gz" \
  -DFROM_HASH="$(openssl dgst -sha256 source.tar.gz | grep -a -i -o -E '[0-9a-f]{64}$')"
make -j3 -C bld-externalproject
