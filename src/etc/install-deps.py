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

# Figure out our target triple
if sys.platform == 'linux' or sys.platform == 'linux2':
    host = host_bits + '-unknown-linux-gnu'
    extra = extra_bits + '-unknown-linux-gnu'
elif sys.platform == 'darwin':
    host = host_bits + '-apple-darwin'
    extra = extra_bits + '-apple-darwin'
elif sys.platform == 'win32':
    if os.environ.get('MSVC') == '1':
        host = host_bits + '-pc-windows-msvc'
        extra = extra_bits + '-pc-windows-msvc'
    else:
        host = host_bits + '-pc-windows-gnu'
else:
    exit_msg = "There is no official Cargo snapshot for {} platform, sorry." 
    sys.exit(exit_msg.format(sys.platform))

rust_date = open('src/rustversion.txt').read().strip()
url = 'https://static.rust-lang.org/dist/' + rust_date


def install_via_tarballs():
    if os.path.isdir("rustc-install"):
        shutil.rmtree("rustc-install")

    # Download the compiler
    host_fname = 'rustc-nightly-' + host + '.tar.gz'
    download.get(url + '/' + host_fname, host_fname)
    download.unpack(host_fname, "rustc-install", quiet=True, strip=2)
    os.remove(host_fname)

    # Download all target libraries needed
    fetch_std(host)
    if extra is not None:
        fetch_std(extra)

    if os.path.isdir("rustc"):
        shutil.rmtree("rustc")
    os.rename("rustc-install", "rustc")

def fetch_std(target):
    fname = 'rust-std-nightly-' + target + '.tar.gz'
    print("adding target libs for " + target)
    download.get(url + '/' + fname, fname)
    download.unpack(fname, "rustc-install", quiet=True, strip=2)
    os.remove(fname)

install_via_tarballs()
