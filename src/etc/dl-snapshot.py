import distutils.spawn
import hashlib
import os
import subprocess
import sys
import tarfile

f = open('src/snapshots.txt')
lines = f.readlines()

date = lines[0]
mac = lines[1]
linux = lines[2]
win = lines[3]

if 'linux' in sys.platform:
    me = linux
elif sys.platform == 'win32':
    me = win
elif sys.platform == 'darwin':
    me = mac
else:
    raise Exception("no snapshot for the platform: " + sys.platform)

platform, hash = me.strip().split(' ')

tarball = 'cargo-nightly-' + platform + '.tar.gz'
url = 'http://static.rust-lang.org/cargo-dist/' + date.strip() + '/' + tarball
dl_path = "target/dl/" + tarball
dst = "target/snapshot"

if not os.path.isdir('target/dl'):
    os.makedirs('target/dl')

ret = subprocess.call(["curl", "-o", dl_path, url])
if ret != 0:
    raise Exception("failed to fetch url")
h = hashlib.sha1(open(dl_path, 'rb').read()).hexdigest()
if h != hash:
    raise Exception("failed to verify the checksum of the snapshot")

tar = tarfile.open(dl_path)
for p in tar.getnames():
    name = p.replace("cargo-nightly/", "", 1)
    fp = os.path.join(dst, name)
    print("extracting " + p)
    tar.extract(p, dst)
tar.close()
