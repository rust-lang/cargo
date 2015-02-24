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

url=https://static-rust-lang-org.s3.amazonaws.com/dist/`cat src/rustversion.txt`

# Install both 64 and 32 bit libraries. Apparently travis barfs if you try to
# just install the right ones? This should enable cross compilation in the
# future anyway.
if [ -z "${windows}" ]; then
    rm -rf rustc *.tar.gz
    curl -O $url/rustc-nightly-i686-$target.tar.gz
    curl -O $url/rustc-nightly-x86_64-$target.tar.gz
    tar xfz rustc-nightly-i686-$target.tar.gz
    tar xfz rustc-nightly-x86_64-$target.tar.gz

    if [ "${BITS}" = "32" ]; then
        src=x86_64
        dst=i686
    else
        src=i686
        dst=x86_64
    fi
    cp -r rustc-nightly-$src-$target/rustc/lib/rustlib/$src-$target \
          rustc-nightly-$dst-$target/rustc/lib/rustlib
    (cd rustc-nightly-$dst-$target && \
     find rustc/lib/rustlib/$src-$target/lib -type f | \
     sed 's/^rustc\//file:/' >> rustc/manifest.in)

    ./rustc-nightly-$dst-$target/install.sh --prefix=rustc
    rm -rf rustc-nightly-$src-$target
    rm -rf rustc-nightly-$dst-$target
    rm -f rustc-nightly-i686-$target.tar.gz
    rm -f rustc-nightly-x86_64-$target.tar.gz
else
    rm -rf rustc *.exe *.tar.gz
    if [ "${BITS}" = "64" ]; then
        triple=x86_64-pc-windows-gnu
    else
        triple=i686-pc-windows-gnu
    fi
    curl -O $url/rustc-nightly-$triple.tar.gz
    tar xfz rustc-nightly-$triple.tar.gz
    ./rustc-nightly-$triple/install.sh --prefix=rustc
    rm -rf rustc-nightly-$triple
    rm -f rustc-nightly-$triple.tar.gz
fi

set +x
