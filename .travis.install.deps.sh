#!/bin/bash

set -x
set -e

if [ "${TRAVIS_OS_NAME}" = "osx" ] || [ "${PLATFORM}" = "mac" ] || [ "`uname`" = "Darwin" ]; then
    target=apple-darwin
elif [ "${OS}" = "Windows_NT" ] || [ "${PLATFORM}" = "win" ]; then
    windows=1
else
    target=unknown-linux-gnu
fi

if [ "${TRAVIS}" = "true" ] && [ "${target}" = "unknown-linux-gnu" ]; then
    # Install a 32-bit compiler for linux
    sudo apt-get update
    if [ "${BITS}" = "32" ]; then
        sudo apt-get install libssl-dev:i386
    fi
    sudo apt-get install g++-multilib lib32stdc++6
fi

url=https://static.rust-lang.org/dist/`cat src/rustversion.txt`

# On unix hosts install both 32 and 64-bit libraries, but respect BITS to
# determine what arch should be used by default. On windows we don't use 32/64
# libraries, but instead we install msvc as an alternate architecture.
if [ -z "${windows}" ]; then
    if [ "${BITS}" = "32" ]; then
        cargo_extra=x86_64-$target
        cargo_host=i686-$target
    else
        cargo_extra=i686-$target
        cargo_host=x86_64-$target
    fi
    libdir=lib
else
    if [ "${BITS}" = "32" ]; then
        cargo_host=i686-pc-windows-gnu
    elif [ "${MSVC}" = "1" ]; then
        cargo_host=x86_64-pc-windows-msvc
    else
        cargo_host=x86_64-pc-windows-gnu
        cargo_extra=x86_64-pc-windows-msvc
    fi
    libdir=bin
fi

rm -rf rustc *.tar.gz
curl -O $url/rustc-nightly-$cargo_host.tar.gz
tar xfz rustc-nightly-$cargo_host.tar.gz

if [ ! -z "${cargo_extra}" ]; then
    curl -O $url/rustc-nightly-$cargo_extra.tar.gz
    tar xfz rustc-nightly-$cargo_extra.tar.gz

    cp -r rustc-nightly-$cargo_extra/rustc/$libdir/rustlib/$cargo_extra \
          rustc-nightly-$cargo_host/rustc/$libdir/rustlib
    cp -r rustc-nightly-$cargo_extra/rustc/$libdir/rustlib/$cargo_extra/bin \
          rustc-nightly-$cargo_host/rustc/$libdir/rustlib/$cargo_host
    (cd rustc-nightly-$cargo_host && \
     find rustc/$libdir/rustlib/$cargo_extra -type f | \
     sed 's/^rustc\//file:/' >> rustc/manifest.in)
    (cd rustc-nightly-$cargo_host && \
     find rustc/$libdir/rustlib/$cargo_host/bin -type f | \
     sed 's/^rustc\//file:/' >> rustc/manifest.in)
    rm -rf rustc-nightly-$cargo_extra
    rm -f rustc-nightly-$cargo_extra.tar.gz
fi

./rustc-nightly-$cargo_host/install.sh --prefix=rustc
rm -rf rustc-nightly-$cargo_host
rm -f rustc-nightly-$cargo_host.tar.gz

set +x
