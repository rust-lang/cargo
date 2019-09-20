#!/usr/bin/env python3

# This script is used to publish Cargo to crates.io.

import os
import re
import subprocess
import urllib.request
from urllib.error import HTTPError


TO_PUBLISH = [
    'crates/cargo-platform',
    'crates/crates-io',
    '.',
]


def already_published(name, version):
    try:
        urllib.request.urlopen('https://crates.io/api/v1/crates/%s/%s/download' % (name, version))
    except HTTPError as e:
        if e.code == 404:
            return False
        raise
    return True


def maybe_publish(path):
    content = open(os.path.join(path, 'Cargo.toml')).read()
    name = re.search('^name = "([^"]+)"', content, re.M).group(1)
    version = re.search('^version = "([^"]+)"', content, re.M).group(1)
    if already_published(name, version):
        print('%s %s is already published, skipping' % (name, version))
        return
    subprocess.check_call(['cargo', 'publish', '--no-verify'], cwd=path)


def main():
    print('Doing dry run first...')
    for path in TO_PUBLISH:
        subprocess.check_call(['cargo', 'publish', '--no-verify', '--dry-run'], cwd=path)
    print('Starting publish...')
    for path in TO_PUBLISH:
        maybe_publish(path)
    print('Publish complete!')


if __name__ == '__main__':
    main()
