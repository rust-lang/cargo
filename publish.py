#!/usr/bin/env python3

# This script is used to publish Cargo to crates.io.
#
# This is run automatically every 6 weeks by the Release team's automation
# whose source is at https://github.com/rust-lang/simpleinfra/.
#
# See https://doc.crates.io/contrib/process/release.html for more about
# Cargo's release process.

import os
import re
import subprocess
import urllib.request
from urllib.error import HTTPError


TO_PUBLISH = [
    'credential/cargo-credential',
    'credential/cargo-credential-libsecret',
    'credential/cargo-credential-wincred',
    'credential/cargo-credential-1password',
    'credential/cargo-credential-macos-keychain',
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
    for path in TO_PUBLISH:
        maybe_publish(path)

    print('Publish complete!')


if __name__ == '__main__':
    main()
