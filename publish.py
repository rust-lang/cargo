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


# Whenever you add a new crate to this list that does NOT start with "cargo-"
# you must reach out to the infra team to add the crate to the list of crates
# allowed to be published from the "cargo CI" crates.io token.
TO_PUBLISH = [
    'credential/cargo-credential',
    'credential/cargo-credential-libsecret',
    'credential/cargo-credential-wincred',
    'credential/cargo-credential-1password',
    'credential/cargo-credential-macos-keychain',
    'crates/rustfix',
    'crates/cargo-platform',
    'crates/cargo-util',
    'crates/crates-io',
    'crates/cargo-util-schemas',
    'crates/cargo-test-macro',
    'crates/cargo-test-support',
    'crates/build-rs',
    '.',
]


def already_published(name, version):
    url = f'https://static.crates.io/crates/{name}/{version}/download'
    try:
        urllib.request.urlopen(url)
    except HTTPError as e:
        # 403 and 404 are common responses to assume it is not published
        if 400 <= e.code < 500:
            return False
        print(f'error: failed to check if {name} {version} is already published')
        print(f'    HTTP response error code {e.code} checking {url}')
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
