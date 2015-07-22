import contextlib
import os
import shutil
import subprocess
import sys
import tarfile

def get(url, path, quiet=False):
    # see http://serverfault.com/questions/301128/how-to-download
    if sys.platform == 'win32':
        run(["PowerShell.exe", "/nologo", "-Command",
             "(New-Object System.Net.WebClient).DownloadFile('" + url +
                "', '" + path + "')"], quiet=quiet)
    else:
        run(["curl", "-o", path, url], quiet=quiet)

def unpack(tarball, dst, quiet=False):
    if quiet:
        print("extracting " + tarball)
    fname = os.path.basename(tarball).replace(".tar.gz", "")
    with contextlib.closing(tarfile.open(tarball)) as tar:
        for p in tar.getnames():
            name = p.replace(fname + "/", "", 1)
            fp = os.path.join(dst, name)
            if not quiet:
                print("extracting " + p)
            tar.extract(p, dst)
            tp = os.path.join(dst, p)
            if os.path.isdir(tp) and os.path.exists(fp):
                continue
            shutil.move(tp, fp)
    shutil.rmtree(os.path.join(dst, fname))

def run(args):
    print("running: " + ' '.join(args))
    ret = subprocess.call(args)
    if ret != 0:
        raise Exception("failed to fetch url: " + url)
