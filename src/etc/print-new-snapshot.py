# When updating snapshots, run this file and pipe it into `src/snapshots.txt`
import os
import subprocess
import sys
import hashlib
import download

date = sys.argv[1]

print(date)

if not os.path.isdir('target/dl'):
    os.makedirs('target/dl')

snaps = {
    'macos-i386': 'i686-apple-darwin',
    'macos-x86_64': 'x86_64-apple-darwin',
    'linux-i386': 'i686-unknown-linux-gnu',
    'linux-x86_64': 'x86_64-unknown-linux-gnu',
    'winnt-i386': 'i686-pc-windows-gnu',
    'winnt-x86_64': 'x86_64-pc-windows-gnu',
    'bitrig-x86_64': 'x86_64-unknown-bitrig',
}

for platform in sorted(snaps):
    triple = snaps[platform]
    tarball = 'cargo-nightly-' + triple + '.tar.gz'
    url = 'https://static.rust-lang.org/cargo-dist/' + date + '/' + tarball
    dl_path = "target/dl/" + tarball
    download.get(url, dl_path, quiet = True)
    h = hashlib.sha1(open(dl_path, 'rb').read()).hexdigest()
    print('  ' + platform + ' ' + h)
