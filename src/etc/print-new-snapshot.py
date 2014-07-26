# When updating snapshots, run this file and pipe it into `src/snapshots.txt`
import os
import subprocess
import sys
import hashlib

date = sys.argv[1]

print(date)

if not os.path.isdir('target/dl'):
    os.makedirs('target/dl')

snaps = ['mac', 'linux', 'win']
for snap in snaps:
    tarball = 'cargo-nightly-' + snap + '.tar.gz'
    url = 'http://static.rust-lang.org/cargo-dist/' + date + '/' + tarball
    dl_path = "target/dl/" + tarball
    ret = subprocess.call(["curl", "-s", "-o", dl_path, url])
    if ret != 0:
        raise Exception("failed to fetch url")
    h = hashlib.sha1(open(dl_path, 'rb').read()).hexdigest()
    print('  ' + snap + ' ' + h)
