import contextlib
import os
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


def unpack(tarball, dst, quiet=False, strip=0):
    if quiet:
        print("extracting " + tarball)
    with contextlib.closing(tarfile.open(tarball)) as tar:
        for p in tar.getmembers():
            if p.isdir():
                continue
            path = []
            p2 = p.name
            while p2 != "":
                a, b = os.path.split(p2)
                path.insert(0, b)
                p2 = a
            if len(path) <= strip:
                continue
            fp = os.path.join(dst, *path[strip:])
            if not quiet:
                print("extracting " + p.name)
            contents = tar.extractfile(p)
            if not os.path.exists(os.path.dirname(fp)):
                os.makedirs(os.path.dirname(fp))
            open(fp, 'wb').write(contents.read())
            os.chmod(fp, p.mode)


def run(args, quiet=False):
    if not quiet:
        print("running: " + ' '.join(args))
    sys.stdout.flush()
    # Use Popen here instead of call() as it apparently allows powershell on
    # Windows to not lock up waiting for input presumably.
    ret = subprocess.Popen(args,
                           stdin=subprocess.PIPE,
                           stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE)
    out, err = ret.communicate()
    code = ret.wait()
    if code != 0:
        print("stdout: \n\n" + out)
        print("stderr: \n\n" + err)
        raise Exception("failed to fetch url")
