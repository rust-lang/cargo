import distutils.spawn
import hashlib
import os
import subprocess
import sys
import tarfile
import shutil

f = open('src/snapshots.txt')
lines = f.readlines()

date = lines[0]
linux32 = lines[1]
linux64 = lines[2]
mac32 = lines[3]
mac64 = lines[4]
win32 = lines[5]
triple = sys.argv[1]

if triple == 'i686-unknown-linux-gnu':
    me = linux32
elif triple == 'x86_64-unknown-linux-gnu':
    me = linux64
elif triple == 'i686-apple-darwin':
    me = mac32
elif triple == 'x86_64-apple-darwin':
    me = mac64
elif triple == 'i686-pc-mingw32':
    me = win32
else:
    raise Exception("no snapshot for the triple: " + triple)

platform, hash = me.strip().split(' ')

tarball = 'cargo-nightly-' + triple + '.tar.gz'
url = 'http://static.rust-lang.org/cargo-dist/' + date.strip() + '/' + tarball
dl_path = "target/dl/" + tarball
dst = "target/snapshot"

if not os.path.isdir('target/dl'):
    os.makedirs('target/dl')

if os.path.isdir(dst):
    shutil.rmtree(dst)

ret = subprocess.call(["curl", "-o", dl_path, url])
if ret != 0:
    raise Exception("failed to fetch url")
h = hashlib.sha1(open(dl_path, 'rb').read()).hexdigest()
if h != hash:
    raise Exception("failed to verify the checksum of the snapshot")

tar = tarfile.open(dl_path)
for p in tar.getnames():
    name = p.replace("cargo-nightly-" + triple + "/", "", 1)
    fp = os.path.join(dst, name)
    print("extracting " + p)
    tar.extract(p, dst)
    shutil.move(os.path.join(dst, p), fp)
tar.close()
shutil.rmtree(os.path.join(dst, 'cargo-nightly-' + triple))
