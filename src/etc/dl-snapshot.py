import download
import hashlib
import os
import re
import shutil
import sys

datere = re.compile('^\d{4}-\d{2}-\d{2}')
cksumre = re.compile('^  ([^ ]+) ([^$]+)$')

current = None
snaps = {}
with open('src/snapshots.txt') as f:
    for line in iter(f):
        line = line.rstrip()
        m = datere.match(line)
        if m:
            current = m.group()
            snaps[current] = {}
            continue

        m = cksumre.match(line)
        if m:
            snaps[current][m.group(1)] = m.group(2)
            continue

        # This script currently doesn't look at older snapshots, so there is
        # no need to look past the first section.
        break

date = current
triple = sys.argv[1]

ts = triple.split('-')
arch = ts[0]

if (arch == 'i586') or (arch == 'i386'):
    arch = 'i686'

if len(ts) == 2:
    vendor = 'unknown'
    target_os = ts[1]
else:
    vendor = ts[1]
    target_os = ts[2]

# NB: The platform format differs from the triple format, to support
#     bootstrapping multiple triples from the same snapshot.
plat_arch = arch if (arch != 'i686') else 'i386'
plat_os = target_os
if (target_os == 'windows'):
    plat_os = 'winnt'
elif (target_os == 'darwin'):
    plat_os = 'macos'
platform = "%s-%s" % (plat_os, plat_arch)
if platform not in snaps[date]:
    raise Exception("no snapshot for the triple '%s'" % triple)

# Reconstitute triple with any applicable changes.  For historical reasons
# this differs from the snapshots.txt platform name.
if target_os == 'linux':
    target_os = 'linux-gnu'
elif target_os == 'darwin':
    vendor = 'apple'
elif target_os == 'windows':
    vendor = 'pc'
    target_os = 'windows-gnu'
triple = "%s-%s-%s" % (arch, vendor, target_os)
hash = snaps[date][platform]

tarball = 'cargo-nightly-' + triple + '.tar.gz'
url = 'https://static.rust-lang.org/cargo-dist/%s/%s' % \
    (date.strip(), tarball)
dl_path = "target/dl/" + tarball
dst = "target/snapshot"

if not os.path.isdir('target/dl'):
    os.makedirs('target/dl')

if os.path.isdir(dst):
    shutil.rmtree(dst)

exists = False
if os.path.exists(dl_path):
    h = hashlib.sha1(open(dl_path, 'rb').read()).hexdigest()
    if h == hash:
        print("file already present %s (%s)" % (dl_path, hash,))
        exists = True

if not exists:
    download.get(url, dl_path)
    h = hashlib.sha1(open(dl_path, 'rb').read()).hexdigest()
    if h != hash:
        raise Exception("failed to verify the checksum of the snapshot")

download.unpack(dl_path, dst, strip=2)
