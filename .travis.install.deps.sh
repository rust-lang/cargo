set -x

if [ "${TRAVIS_OS_NAME}" = "osx" ] || [ "${PLATFORM}" = "mac" ]; then
    target=apple-darwin
elif [ "${OS}" = "Windows_NT" ] || [ "${PLATFORM}" = "win" ]; then
    target=pc-mingw32
    windows=1
elif [ "${TRAVIS_OS_NAME}" = "linux" ] || [ "${PLATFORM}" = "linux" ] ||
     [ "${TRAVIS_OS_NAME}" = "" ]; then
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

# Install both 64 and 32 bit libraries. Apparently travis barfs if you try to
# just install the right ones? This should enable cross compilation in the
# future anyway.
if [ -z "${windows}" ]; then
    curl -O https://static-rust-lang-org.s3.amazonaws.com/dist/rust-nightly-i686-$target.tar.gz
    tar xfz rust-nightly-i686-$target.tar.gz
    curl -O https://static-rust-lang-org.s3.amazonaws.com/dist/rust-nightly-x86_64-$target.tar.gz
    tar xfz rust-nightly-x86_64-$target.tar.gz

    if [ "${BITS}" = "32" ]; then
        src=x86_64
        dst=i686
    else
        src=i686
        dst=x86_64
    fi
    cp -r rust-nightly-$src-$target/lib/rustlib/$src-$target \
          rust-nightly-$dst-$target/lib/rustlib
    (cd rust-nightly-$dst-$target && \
     find lib/rustlib/$src-$target/lib -type f >> \
     lib/rustlib/manifest.in)

    ./rust-nightly-$dst-$target/install.sh --prefix=rustc
    rm -rf rust-nightly-$src-$target
    rm -rf rust-nightly-$dst-$target
else
    rm -rf *.exe rustc
    curl -O http://static-rust-lang-org.s3.amazonaws.com/dist/rust-nightly-install.exe
    innounp -y -x rust-nightly-install.exe
    mv '{app}' rustc
fi

set +x
