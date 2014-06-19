set -ex

# Install a 32-bit compiler for linux
sudo apt-get update
sudo apt-get install gcc-multilib
target=unknown-linux-gnu

# Install both 64 and 32 bit libraries. Apparently travis barfs if you try to
# just install the right ones? This should enable cross compilation in the
# future anyway.
curl -O http://static.rust-lang.org/dist/rust-nightly-x86_64-$target.tar.gz
curl -O http://static.rust-lang.org/dist/rust-nightly-i686-$target.tar.gz
tar xfz rust-nightly-x86_64-$target.tar.gz
tar xfz rust-nightly-i686-$target.tar.gz
cp -r rust-nightly-i686-$target/lib/rustlib/i686-$target \
      rust-nightly-x86_64-$target/lib/rustlib
(cd rust-nightly-x86_64-$target && \
 find lib/rustlib/i686-$target/lib -type f >> \
 lib/rustlib/manifest.in)
sudo ./rust-nightly-x86_64-$target/install.sh

