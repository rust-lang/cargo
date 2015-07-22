#!/usr/bin/env python

import contextlib
import download
import os
import shutil
import sys
import tarfile

if os.environ.get('BITS') == '32':
    host_bits = 'i686'
    extra_bits = 'x86_64'
else:
    host_bits = 'x86_64'
    extra_bits = 'i686'

extra = None
libdir = 'lib'

# Figure out our target triple
if sys.platform == 'linux' or sys.platform == 'linux2':
    host = host_bits + '-unknown-linux-gnu'
    extra = extra_bits + '-unknown-linux-gnu'
elif sys.platform == 'darwin':
    host = host_bits + '-apple-darwin'
    extra = extra_bits + '-apple-darwin'
elif sys.platform == 'win32':
    libdir = 'bin'
    if os.environ.get('MSVC') == '1':
        host = host_bits + '-pc-windows-msvc'
        extra = extra_bits + '-pc-windows-msvc'
    else:
        host = host_bits + '-pc-windows-gnu'
else:
    raise "Unknown platform"

rust_date = open('src/rustversion.txt').read().strip()
url = 'https://static.rust-lang.org/dist/' + rust_date

def install_via_tarballs():
    if os.path.isdir("rustc-install"):
        shutil.rmtree("rustc-install")

    host_fname = 'rustc-nightly-' + host + '.tar.gz'
    download.get(url + '/' + host_fname, host_fname)
    download.unpack(host_fname, "rustc-install", quiet=True)
    os.remove(host_fname)

    if extra != None:
        extra_fname = 'rustc-nightly-' + extra + '.tar.gz'
        print("adding target libs for " + extra)
        download.get(url + '/' + extra_fname, extra_fname)
        folder = extra_fname.replace(".tar.gz", "")
        with contextlib.closing(tarfile.open(extra_fname)) as tar:
            for p in tar.getnames():
                if not "rustc/" + libdir + "/rustlib/" + extra in p:
                    continue
                name = p.replace(folder + "/", "", 1)
                dst = "rustc-install/" + name
                tar.extract(p, "rustc-install")
                tp = os.path.join("rustc-install", p)
                if os.path.isdir(tp) and os.path.exists(dst):
                    continue
                shutil.move(tp, dst)
        shutil.rmtree("rustc-install/" + folder)
        os.remove(extra_fname)

    if os.path.isdir("rustc"):
        shutil.rmtree("rustc")
    os.rename("rustc-install/rustc", "rustc")
    shutil.rmtree("rustc-install")

install_via_tarballs()
