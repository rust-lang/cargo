#!/usr/bin/env bash

set -eu

# This script is called by the oss-fuzz main project when compiling the fuzz
# targets. This script is regression tested by ci/ossfuzz.sh.

# Save off the current folder as the build root.
export BUILD_ROOT="$PWD"

echo "CC: $CC"
echo "CXX: $CXX"
echo "LIB_FUZZING_ENGINE: $LIB_FUZZING_ENGINE"
echo "CFLAGS: $CFLAGS"
echo "CXXFLAGS: $CXXFLAGS"
echo "OUT: $OUT"

MAKEFLAGS+="-j$(nproc)"
export MAKEFLAGS

# Install dependencies
apt-get -y install automake libtool libssl-dev zlib1g-dev

# Compile the fuzzer.
autoreconf -fi
./configure --disable-shared \
            --enable-ossfuzzers \
            --disable-examples-build \
            --enable-debug
make V=1

# Copy the fuzzer to the output directory.
cp -v tests/ossfuzz/ssh2_client_fuzzer "$OUT/"
