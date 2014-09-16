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

host=static-rust-lang-org.s3.amazonaws.com

# Install both 64 and 32 bit libraries. Apparently travis barfs if you try to
# just install the right ones? This should enable cross compilation in the
# future anyway.
if [ -z "${windows}" ]; then
    curl -O https://$host/dist/rust-nightly-i686-$target.tar.gz
    curl -O https://$host/dist/rust-nightly-x86_64-$target.tar.gz
    tar xfz rust-nightly-i686-$target.tar.gz
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
    rm -f rust-nightly-i686-$target.tar.gz
    rm -f rust-nightly-x86_64-$target.tar.gz
else
    rm -rf *.exe rustc
    # Right now we don't have *cargo* nightlies for 64-bit windows. This means
    # that to bootstrap the 64-bit cargo nightlies, we need to build from the
    # 32-bit cargo. This, however, has a runtime dependency on libgcc_s_dw2
    # which is not present in the mingw-w64 64-bit shell. Hence we download both
    # *rust* snapshots, and then when we're on a 64-bit windows host we copy the
    # libgcc_s_dw2 dll from the 32-bit rust nightly into a location that will be
    # in our PATH
    #
    # When cargo has a 64-bit nightly of its own, we'll only need to download
    # the relevant windows nightly.
    v32=i686-w64-mingw32
    v64=x86_64-w64-mingw32
    curl -O http://$host/dist/rust-nightly-$v32.exe
    curl -O http://$host/dist/rust-nightly-$v64.exe
    if [ "${BITS}" = "64" ]; then
        # innoextract comes from the mingw-w64-x86_64-innoextract package
        innoextract rust-nightly-$v64.exe
        mv app rustc
        innoextract rust-nightly-$v32.exe
        mv app/bin/libgcc_s_dw2-1.dll rustc/bin
        rm -rf app
    else
        # innounp came from a random download on the internet! (search google)
        innounp -y -x rust-nightly-$v32.exe
        mv '{app}' rustc
    fi
    rm -f rust-nightly-$v32.exe
    rm -f rust-nightly-$v64.exe
fi

set +x
