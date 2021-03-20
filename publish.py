#!/usr/bin/env python3

# This script is used to publish Cargo to crates.io.

import os
import re
import subprocess
import time
import urllib.request
from urllib.error import HTTPError


TO_PUBLISH = [
    'crates/cargo-platform',
    'crates/cargo-util',
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
        return False
    subprocess.check_call(['cargo', 'publish', '--no-verify'], cwd=path)
    return True


def main():
    print('Starting publish...')
    for i, path in enumerate(TO_PUBLISH):
        if maybe_publish(path):
            if i < len(TO_PUBLISH)-1:
                # Sleep to allow the index to update. This should probably
                # check that the index is updated, or use a retry loop
                # instead.
                time.sleep(5)
    print('Publish complete!')


if __name__ == '__main__':
    main()
