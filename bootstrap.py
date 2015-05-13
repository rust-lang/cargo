#!/usr/bin/env python
"""
About
=====

This python script is design to do the bare minimum to compile and link the
cargo binary for the purposes of bootstrapping itself on a new platform.  All
that should be necessary to run this is a working Rust toolchain, Python, and
Git.

This script will not set up a full cargo cache or anything.  It works by
cloning the cargo index and then starting with the cargo dependencies, it
recursively builds the dependency tree.  Once it has the dependency tree, it
starts with the leaves of the tree, doing a breadth first traversal and for
each dependency, it clones the repo, sets the repo's head to the correct
revision and then executes the build command specified in the cargo config.

This bootstrap script uses a temporary directory to store the built dependency
libraries and uses that as a link path when linking dependencies and the
cargo binary.  The goal is to create a statically linked cargo binary that is
capable of being used as a "local cargo" when running the main cargo Makefiles.

Dependencies
============

* pytoml -- used for parsing toml files.
  https://github.com/avakar/pytoml

* dulwich -- used for working with git repos.
  https://git.samba.org/?p=jelmer/dulwich.git;a=summary

Both can be installed via the pip tool:

$ sudo pip install pytoml dulwich

Command Line Options
====================

--cargo-root <path>    specify the path to the cargo repo root.
--target-dir <path>    specify the location to store build results.

The cargo root option defaults to the current directory if unspecified.  The
target directory defaults to Python equivilent of 'mktemp -d' if unspecified.
"""

import argparse, \
       cStringIO, \
       hashlib, \
       httplib, \
       inspect, \
       json, \
       os, \
       re, \
       shutil, \
       subprocess, \
       sys, \
       tarfile, \
       tempfile, \
       urlparse
import pytoml as toml
import dulwich.porcelain as git

CRATES_INDEX = 'git://github.com/rust-lang/crates.io-index.git'
CARGO_REPO = 'git://github.com/rust-lang/cargo.git'
CRATE_API_DL = 'https://crates.io/api/v1/crates/%s/%s/download'
SV_RANGE = re.compile('^(?P<op>(?:\<|\>|=|\<=|\>=|\^|\~))?'
                      '(?P<major>(?:\*|0|[1-9][0-9]*))'
                      '(\.(?P<minor>(?:\*|0|[1-9][0-9]*)))?'
                      '(\.(?P<patch>(?:\*|0|[1-9][0-9]*)))?'
                      '(\-(?P<prerelease>[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*))?'
                      '(\+(?P<build>[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*))?$')
SEMVER = re.compile('(?P<major>(?:0|[1-9][0-9]*))'
                    '(\.(?P<minor>(?:0|[1-9][0-9]*)))?'
                    '(\.(?P<patch>(?:0|[1-9][0-9]*)))?'
                    '(\-(?P<prerelease>[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*))?'
                    '(\+(?P<build>[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*))?$')
BUILT = {}
CRATES = {}
UNRESOLVED = []


class PreRelease(object):

    def __init__(self, pr):
        self._container = []
        if pr is not None:
            self._container += str(pr).split('.')

    def __str__(self):
        return '.'.join(self._container)

    def __repr__(self):
        return self._container

    def __getitem__(self, key):
        return self._container[key]

    def __len__(self):
        return len(self._container)

    def __gt__(self, rhs):
        return not ((self < rhs) or (self == rhs))

    def __ge__(self, rhs):
        return not (self < rhs)

    def __le__(self, rhs):
        return not (self > rhs)

    def __eq__(self, rhs):
        return self._container == rhs._container

    def __ne__(self, rhs):
        return not (self == rhs)

    def __lt__(self, rhs):
        if self == rhs:
            return False

        # not having a pre-release is higher precedence
        if len(self) == 0:
            if len(rhs) == 0:
                return False
            else:
                # 1.0.0 > 1.0.0-alpha
                return False
        else:
            if len(rhs) is None:
                # 1.0.0-alpha < 1.0.0
                return True

        # if both have one, then longer pre-releases are higher precedence
        if len(self) > len(rhs):
            # 1.0.0-alpha.1 > 1.0.0-alpha
            return False
        elif len(self) < len(rhs):
            # 1.0.0-alpha < 1.0.0-alpha.1
            return True

        # if both have the same length pre-release, must check each piece
        # numeric sub-parts have lower precedence than non-numeric sub-parts
        # non-numeric sub-parts are compared lexically in ASCII sort order
        for l,r in zip(self, rhs):
            if l.isdigit():
                if r.isdigit():
                    if int(l) < int(r):
                        # 2 > 1
                        return True
                    elif int(l) > int(r):
                        # 1 < 2
                        return False
                    else:
                        # 1 == 1
                        continue
                else:
                    # 1 < 'foo'
                    return True
            else:
                if r.isdigit():
                    # 'foo' > 1
                    return False

            # both are non-numeric
            if l < r:
                return True
            elif l > r:
                return False

        raise RuntimeError('PreRelease __lt__ failed')


class Semver(dict):

    def __init__(self, sv):
        match = SEMVER.match(str(sv))
        if match is None:
            raise ValueError('%s is not a valid semver string' % sv)

        self._input = sv
        self.update(match.groupdict())
        self.prerelease = PreRelease(self['prerelease'])

    def __str__(self):
        major, minor, patch, prerelease, build = self.parts_raw()
        s = ''
        if major is None:
            s += '0'
        else:
            s += major
        s += '.'
        if minor is None:
            s += '0'
        else:
            s += minor
        s += '.'
        if patch is None:
            s += '0'
        else:
            s += patch
        if len(self.prerelease):
            s += '-' + str(self.prerelease)
        if build is not None:
            s += '+' + build
        return s

    def __hash__(self):
        return hash(str(self))

    def parts(self):
        major, minor, patch, prerelease, build = self.parts_raw()
        if major is None:
            major = '0'
        if minor is None:
            minor = '0'
        if patch is None:
            patch = '0'
        return (int(major),int(minor),int(patch),prerelease,build)

    def parts_raw(self):
        return (self['major'],self['minor'],self['patch'],self['prerelease'],self['build'])

    def __lt__(self, rhs):
        lmaj,lmin,lpat,lpre,_ = self.parts()
        rmaj,rmin,rpat,rpre,_ = rhs.parts()
        if lmaj < rmaj:
            return True
        elif lmin < rmin:
            return True
        elif lpat < rpat:
            return True
        elif lpre is not None and rpre is None:
            return True
        elif lpre is not None and rpre is not None:
            if self.prerelease < rhs.prerelease:
                return True
        return False

    def __le__(self, rhs):
        return not (self > rhs)

    def __gt__(self, rhs):
        return not ((self < rhs) or (self == rhs))

    def __ge__(self, rhs):
        return not (self < rhs)

    def __eq__(self, rhs):
        # build metadata is only considered for equality
        lmaj,lmin,lpat,lpre,lbld = self.parts()
        rmaj,rmin,rpat,rpre,rbld = rhs.parts()
        return lmaj == rmaj and \
               lmin == rmin and \
               lpat == rpat and \
               lpre == rpre and \
               lbld == rbld

    def __ne__(self, rhs):
        return not (self == rhs)


class SemverRange(dict):

    def __init__(self, sv):
        match = SV_RANGE.match(str(sv))
        if match is None:
            raise ValueError('%s is not a valid semver range string' % sv)

        self._input = sv
        self.update(match.groupdict())
        self.prerelease = PreRelease(self['prerelease'])

        # fix up the op
        op = self['op']
        if op is None:
            if self['major'] == '*' or self['minor'] == '*' or self['patch'] == '*':
                op = '*'
            else:
                # if no op was specified and there are no wildcards, then op
                # defaults to '^'
                op = '^'
        else:
            self._semver = Semver(sv[len(op):])

        if op not in ('<=', '>=', '<', '>', '=', '^', '~', '*'):
            raise ValueError('%s is not a valid semver operator' % op)

        self['op'] = op

    def parts_raw(self):
        return (self['major'],self['minor'],self['patch'],self['prerelease'],self['build'])

    def __str__(self):
        major, minor, patch, prerelease, build = self.parts_raw()
        s = self['op']
        if major is None:
            s += '0'
        else:
            s += major
        s += '.'
        if minor is None:
            s += '0'
        else:
            s += minor
        s += '.'
        if patch is None:
            s += '0'
        else:
            s += patch
        if len(self.prerelease):
            s += '-' + str(self.prerelease)
        if build is not None:
            s += '+' + build
        return s

    def lower(self):
        op = self['op']
        major,minor,patch,_,_ = self.parts_raw()

        if op in ('<=', '<', '=', '>', '>='):
            return None

        if op == '*':
            # wildcards specify a range
            if self['major'] == '*':
                return Semver('0.0.0')
            elif self['minor'] == '*':
                return Semver(major + '.0.0')
            elif self['patch'] == '*':
                return Semver(major + '.' + minor + '.0')
        elif op == '^':
            # caret specifies a range
            if patch is None:
                if minor is None:
                    # ^0 means >=0.0.0 and <1.0.0
                    return Semver(major + '.0.0')
                else:
                    # ^0.0 means >=0.0.0 and <0.1.0
                    return Semver(major + '.' + minor + '.0')
            else:
                # ^0.0.1 means >=0.0.1 and <0.0.2
                # ^0.1.2 means >=0.1.2 and <0.2.0
                # ^1.2.3 means >=1.2.3 and <2.0.0
                if int(major) == 0:
                    if int(minor) == 0:
                        # ^0.0.1
                        return Semver('0.0.' + patch)
                    else:
                        # ^0.1.2
                        return Semver('0.' + minor + '.' + patch)
                else:
                    # ^1.2.3
                    return Semver(major + '.' + minor + '.' + patch)
        elif op == '~':
            # tilde specifies a minimal range
            if patch is None:
                if minor is None:
                    # ~0 means >=0.0.0 and <1.0.0
                    return Semver(major + '.0.0')
                else:
                    # ~0.0 means >=0.0.0 and <0.1.0
                    return Semver(major + '.' + minor + '.0')
            else:
                # ~0.0.1 means >=0.0.1 and <0.1.0
                # ~0.1.2 means >=0.1.2 and <0.2.0
                # ~1.2.3 means >=1.2.3 and <1.3.0
                return Semver(major + '.' + minor + '.' + patch)

        raise RuntimeError('No lower bound')

    def upper(self):
        op = self['op']
        major,minor,patch,_,_ = self.parts_raw()

        if op in ('<=', '<', '=', '>', '>='):
            return None

        if op == '*':
            # wildcards specify a range
            if self['major'] == '*':
                return None
            elif self['minor'] == '*':
                return Semver(str(int(major) + 1) + '.0.0')
            elif self['patch'] == '*':
                return Semver(major + '.' + str(int(minor) + 1) + '.0')
        elif op == '^':
            # caret specifies a range
            if patch is None:
                if minor is None:
                    # ^0 means >=0.0.0 and <1.0.0
                    return Semver(str(int(major) + 1) + '.0.0')
                else:
                    # ^0.0 means >=0.0.0 and <0.1.0
                    return Semver(major + '.' + str(int(minor) + 1) + '.0')
            else:
                # ^0.0.1 means >=0.0.1 and <0.0.2
                # ^0.1.2 means >=0.1.2 and <0.2.0
                # ^1.2.3 means >=1.2.3 and <2.0.0
                if int(major) == 0:
                    if int(minor) == 0:
                        # ^0.0.1
                        return Semver('0.0.' + str(int(patch) + 1))
                    else:
                        # ^0.1.2
                        return Semver('0.' + str(int(minor) + 1) + '.0')
                else:
                    # ^1.2.3
                    return Semver(str(int(major) + 1) + '.0.0')
        elif op == '~':
            # tilde specifies a minimal range
            if patch is None:
                if minor is None:
                    # ~0 means >=0.0.0 and <1.0.0
                    return Semver(str(int(major) + 1) + '.0.0')
                else:
                    # ~0.0 means >=0.0.0 and <0.1.0
                    return Semver(major + '.' + str(int(minor) + 1) + '.0')
            else:
                # ~0.0.1 means >=0.0.1 and <0.1.0
                # ~0.1.2 means >=0.1.2 and <0.2.0
                # ~1.2.3 means >=1.2.3 and <1.3.0
                return Semver(major + '.' + str(int(minor) + 1) + '.0')

        raise RuntimeError('No upper bound')

    def compare(self, sv):
        if type(sv) is not Semver:
            sv = Semver(sv)

        op = self['op']
        major,minor,patch,_,_ = self.parts_raw()

        if op == '*':
            if self['major'] == '*':
                return sv >= Semver('0.0.0')

            return (sv >= self.lower()) and (sv < self.upper())
        elif op == '^':
            return (sv >= self.lower()) and (sv < self.upper())
        elif op == '~':
            return (sv >= self.lower()) and (sv < self.upper())
        elif op == '<=':
            return sv <= self._semver
        elif op == '>=':
            return sv >= self._semver
        elif op == '<':
            return sv < self._semver
        elif op == '>':
            return sv > self._semver
        elif op == '=':
            return sv == self._semver

        raise RuntimeError('Semver comparison failed to find a matching op')

def test_semver():
    print '\ntesting parsing:'
    print '"1"                    is: "%s"' % Semver("1")
    print '"1.1"                  is: "%s"' % Semver("1.1")
    print '"1.1.1"                is: "%s"' % Semver("1.1.1")
    print '"1.1.1-alpha"          is: "%s"' % Semver("1.1.1-alpha")
    print '"1.1.1-alpha.1"        is: "%s"' % Semver("1.1.1-alpha.1")
    print '"1.1.1-alpha+beta"     is: "%s"' % Semver("1.1.1-alpha+beta")
    print '"1.1.1-alpha.1+beta"   is: "%s"' % Semver("1.1.1-alpha.1+beta")
    print '"1.1.1-alpha.1+beta.1" is: "%s"' % Semver("1.1.1-alpha.1+beta.1")

    print '\ntesting equality:'
    print '"1"                    == "1.0.0"                is: %s' % (Semver("1") == Semver("1.0.0"))
    print '"1.1"                  == "1.1.0"                is: %s' % (Semver("1.1") == Semver("1.1.0"))
    print '"1.1.1"                == "1.1.1"                is: %s' % (Semver("1.1.1") == Semver("1.1.1"))
    print '"1.1.1-alpha"          == "1.1.1-alpha"          is: %s' % (Semver("1.1.1-alpha") == Semver("1.1.1-alpha"))
    print '"1.1.1-alpha.1"        == "1.1.1-alpha.1"        is: %s' % (Semver("1.1.1-alpha.1") == Semver("1.1.1-alpha.1"))
    print '"1.1.1-alpha+beta"     == "1.1.1-alpha+beta"     is: %s' % (Semver("1.1.1-alpha+beta") == Semver("1.1.1-alpha+beta"))
    print '"1.1.1-alpha.1+beta"   == "1.1.1-alpha.1+beta"   is: %s' % (Semver("1.1.1-alpha.1+beta") == Semver("1.1.1-alpha.1+beta"))
    print '"1.1.1-alpha.1+beta.1" == "1.1.1-alpha.1+beta.1" is: %s' % (Semver("1.1.1-alpha.1+beta.1") == Semver("1.1.1-alpha.1+beta.1"))

    print '\ntesting less than:'
    print '"1"                  < "2.0.0"              is: %s' % (Semver("1") < Semver("2.0.0"))
    print '"1.1"                < "1.2.0"              is: %s' % (Semver("1.1") < Semver("1.2.0"))
    print '"1.1.1"              < "1.1.2"              is: %s' % (Semver("1.1.1") < Semver("1.1.2"))
    print '"1.1.1-alpha"        < "1.1.1"              is: %s' % (Semver("1.1.1-alpha") < Semver("1.1.1"))
    print '"1.1.1-alpha"        < "1.1.1-beta"         is: %s' % (Semver("1.1.1-alpha") < Semver("1.1.1-beta"))
    print '"1.1.1-1"            < "1.1.1-alpha"        is: %s' % (Semver("1.1.1-alpha") < Semver("1.1.1-beta"))
    print '"1.1.1-alpha"        < "1.1.1-alpha.1"      is: %s' % (Semver("1.1.1-alpha") < Semver("1.1.1-alpha.1"))
    print '"1.1.1-alpha.1"      < "1.1.1-alpha.2"      is: %s' % (Semver("1.1.1-alpha.1") < Semver("1.1.1-alpha.2"))
    print '"1.1.1-alpha+beta"   < "1.1.1+beta"         is: %s' % (Semver("1.1.1-alpha+beta") < Semver("1.1.1+beta"))
    print '"1.1.1-alpha+beta"   < "1.1.1-beta+beta"    is: %s' % (Semver("1.1.1-alpha+beta") < Semver("1.1.1-beta+beta"))
    print '"1.1.1-1+beta"       < "1.1.1-alpha+beta"   is: %s' % (Semver("1.1.1-alpha+beta") < Semver("1.1.1-beta+beta"))
    print '"1.1.1-alpha+beta"   < "1.1.1-alpha.1+beta" is: %s' % (Semver("1.1.1-alpha+beta") < Semver("1.1.1-alpha.1+beta"))
    print '"1.1.1-alpha.1+beta" < "1.1.1-alpha.2+beta" is: %s' % (Semver("1.1.1-alpha.1+beta") < Semver("1.1.1-alpha.2+beta"))

    print '\ntesting semver range parsing:'
    print '"0"      lower: %s, upper: %s' % (SemverRange('0').lower(), SemverRange('0').upper())
    print '"0.0"    lower: %s, upper: %s' % (SemverRange('0.0').lower(), SemverRange('0.0').upper())
    print '"0.0.0"  lower: %s, upper: %s' % (SemverRange('0.0.0').lower(), SemverRange('0.0.0').upper())
    print '"0.0.1"  lower: %s, upper: %s' % (SemverRange('0.0.1').lower(), SemverRange('0.0.1').upper())
    print '"0.1.1"  lower: %s, upper: %s' % (SemverRange('0.1.1').lower(), SemverRange('0.1.1').upper())
    print '"1.1.1"  lower: %s, upper: %s' % (SemverRange('1.1.1').lower(), SemverRange('1.1.1').upper())
    print '"^0"     lower: %s, upper: %s' % (SemverRange('^0').lower(), SemverRange('^0').upper())
    print '"^0.0"   lower: %s, upper: %s' % (SemverRange('^0.0').lower(), SemverRange('^0.0').upper())
    print '"^0.0.0" lower: %s, upper: %s' % (SemverRange('^0.0.0').lower(), SemverRange('^0.0.0').upper())
    print '"^0.0.1" lower: %s, upper: %s' % (SemverRange('^0.0.1').lower(), SemverRange('^0.0.1').upper())
    print '"^0.1.1" lower: %s, upper: %s' % (SemverRange('^0.1.1').lower(), SemverRange('^0.1.1').upper())
    print '"^1.1.1" lower: %s, upper: %s' % (SemverRange('^1.1.1').lower(), SemverRange('^1.1.1').upper())
    print '"~0"     lower: %s, upper: %s' % (SemverRange('~0').lower(), SemverRange('~0').upper())
    print '"~0.0"   lower: %s, upper: %s' % (SemverRange('~0.0').lower(), SemverRange('~0.0').upper())
    print '"~0.0.0" lower: %s, upper: %s' % (SemverRange('~0.0.0').lower(), SemverRange('~0.0.0').upper())
    print '"~0.0.1" lower: %s, upper: %s' % (SemverRange('~0.0.1').lower(), SemverRange('~0.0.1').upper())
    print '"~0.1.1" lower: %s, upper: %s' % (SemverRange('~0.1.1').lower(), SemverRange('~0.1.1').upper())
    print '"~1.1.1" lower: %s, upper: %s' % (SemverRange('~1.1.1').lower(), SemverRange('~1.1.1').upper())
    print '"*"      lower: %s, upper: %s' % (SemverRange('*').lower(), SemverRange('*').upper())
    print '"0.*"    lower: %s, upper: %s' % (SemverRange('0.*').lower(), SemverRange('0.*').upper())
    print '"0.0.*"  lower: %s, upper: %s' % (SemverRange('0.0.*').lower(), SemverRange('0.0.*').upper())


class Crate(object):

    def __init__(self, crate, ver, deps, cdir, build):
        self._crate = str(crate)
        self._version = Semver(ver)
        self._dep_info = deps
        self._dir = cdir
        self._build = build
        self._resolved = False
        self._deps = {}
        self._refs = []

    def name(self):
        return self._crate

    def version(self):
        return self._version

    def __str__(self):
        return '%s-%s' % (self.name(), self.version())

    def add_dep(self, crate, features):
        if self._deps.has_key(str(crate)):
            return

        self._deps[str(crate)] = { 'features': features }
        crate.add_ref(self)

    def add_ref(self, crate):
        if str(crate) not in self._refs:
            self._refs.append(str(crate))

    def resolved(self):
        return self._resolved

    def resolve(self, tdir, idir):
        global CRATES
        global UNRESOLVED

        if self._resolved:
            return
        if CRATES.has_key(str(self)):
            return

        if self._dep_info is not None:
            print '\nResolving dependencies for: %s' % str(self)
            for d in self._dep_info:
                kind = d.get('kind', 'normal')
                if kind != 'normal':
                    print 'skipping non-normal dep %s' % d['name']
                    continue

                optional = d.get('optional', False)
                if optional:
                    print 'skipping optional dep %s' % d['name']
                    continue

                svr = SemverRange(d['req'])
                print '\nLooking for %s %s' % (d['name'], str(svr))
                name, ver, deps, ftrs, cksum = crate_info_from_index(idir, d['name'], svr)
                cdir = dl_and_check_crate(tdir, name, ver, cksum)
                _, _, _, build = crate_info_from_toml(cdir)
                try:
                    dcrate = Crate(name, ver, deps, cdir, build)
                    UNRESOLVED.append(dcrate)
                except:
                    dcrate = None

                # clean up the list of features that are enabled
                tftrs = d.get('features', [])
                if type(tftrs) is dict:
                    tftrs = tftrs.keys()
                else:
                    tftrs = filter(lambda x: len(x) > 0, tftrs)

                # add 'default' if default_features is true
                if d.get('default_features', True):
                    tftrs.append('default')

                features = []
                if type(ftrs) is dict:
                    # add any available features that are activated by the
                    # dependency entry in the parent's dependency record
                    for k in tftrs:
                        if ftrs.has_key(k):
                            features += ftrs[k]
                else:
                    features += filter(lambda x: (len(x) > 0) and (x in tftrs), ftrs)

                if dcrate is not None:
                    self.add_dep(dcrate, features)

        self._resolved = True
        CRATES[str(self)] = self

    def build(self, by, out_dir, features=[]):
        global BUILT
        global CRATES

        if BUILT.has_key(str(self)):
            return

        for dep,info in self._deps.iteritems():
            if CRATES.has_key(dep):
                CRATES[dep].build(self, out_dir, info['features'])

        print 'building %s with features: %s (needed by: %s)' % (str(self), features, ', '.join(self._refs))

        cmds = []
        for b in self._build:
            v = str(self._version).replace('.','_')
            cmd = ['rustc']
            cmd.append(os.path.join(self._dir, b['path']))
            cmd.append('--crate-name')
            if b['type'] == 'lib':
                cmd.append(b['name'])
                cmd.append('--crate-type')
                cmd.append('lib')
            elif b['type'] == 'build_script':
                cmd.append('build_script_%s_%s' % (b['name'], v))
                cmd.append('--crate-type')
                cmd.append('bin')
            else:
                cmd.append(b['name'])
                cmd.append('--crate-type')
                cmd.append('bin')

            for f in features:
                cmd.append('--cfg')
                cmd.append('feature="%s"' % f)

            cmd.append('--out-dir')
            cmd.append('%s' % out_dir)
            cmd.append('-L')
            cmd.append('%s' % out_dir)
            cmds.append(cmd)

            if b['type'] == 'build_script':
                cmds.append([os.path.join(out_dir, 'build_script_%s_%s' % (b['name'], v))])

        for c in cmds:
            print ' %s' % ' '.join(c)
            proc = subprocess.Popen(c, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            (out, err) = proc.communicate()
            out = out.split('\n')
            err = err.split('\n')
            for o in out:
                if len(o) > 1:
                    print ' %s' % o
            for e in err:
                if len(e) > 1:
                    print ' %s' % e

        BUILT[str(self)] = str(by)

def dl_crate(url, depth=0):
    if depth > 10:
        raise RuntimeError('too many redirects')

    loc = urlparse.urlparse(url)
    if loc.scheme == 'https':
        conn = httplib.HTTPSConnection(loc.netloc)
    elif loc.scheme == 'http':
        conn = httplib.HTTPConnection(loc.netloc)
    else:
        raise RuntimeError('unsupported url scheme: %s' % loc.scheme)

    conn.request("GET", loc.path)
    res = conn.getresponse()
    print '%sconnected to %s...%s' % ((' ' * depth), url, res.status)
    headers = dict(res.getheaders())
    if headers.has_key('location') and headers['location'] != url:
        return dl_crate(headers['location'], depth + 1)

    return res.read()

def dl_and_check_crate(tdir, name, ver, cksum):
    global CRATES
    try:
        cname = '%s-%s' % (name, ver)
        cdir = os.path.join(tdir, cname)
        if CRATES.has_key(cname):
            print 'skipping %s...already downloaded' % cname
            return

        if not os.path.isdir(cdir):
            print 'Downloading %s source to %s' % (cname, cdir)
            dl = CRATE_API_DL % (name, ver)
            buf = dl_crate(dl)
            if (cksum is not None):
                h = hashlib.sha256()
                h.update(buf)
                if h.hexdigest() == cksum:
                    print 'Checksum is good...%s' % cksum
                else:
                    print 'Checksum is BAD (%s != %s)' % (h.hexdigest(), cksum)

            fbuf = cStringIO.StringIO(buf)
            with tarfile.open(fileobj=fbuf) as tf:
                print 'unpacking result to %s...' % cdir
                tf.extractall(path=tdir)

    except Exception, e:
        self._dir = None
        raise e

    return cdir

def crate_info_from_toml(cdir):
    try:
        with open(os.path.join(cdir, 'Cargo.toml'), 'rb') as ctoml:
            cfg = toml.load(ctoml)
            build = []
            p = cfg.get('package',cfg.get('project', {}))
            name = p.get('name', None)
            ver = p.get('version', None)
            if (name is None) or (ver is None):
                raise RuntimeError('invalid .toml file format')

            # look for a "build" item
            bf = p.get('build', None)
            if bf is not None:
                if bf == 'lib.rs':
                    bftype = 'lib'
                else:
                    bftype = 'build_script'
                build.append({'type': bftype, 'path':bf, 'name':name.replace('-','_')})

            # look for libs array
            libs = cfg.get('lib', [])
            if type(libs) is not list:
                libs = [libs]
            for l in libs:
                l['type'] = 'lib'
                build.append(l)

            # look for bins array
            bins = cfg.get('bin', [])
            if type(bins) is not list:
                bins = [bins]
            for b in bins:
                tb = {'type': 'bin', 'name':b['name'], 'path':os.path.join('bin', '%s.rs'%b['name'])}
                build.append(tb)

            # if no explicit directions on what to build, then add a default
            if len(build) == 0:
                build.append({'type':'lib', 'path':'lib.rs', 'name':name.replace('-','_')})

            for b in build:
                bpath = os.path.join(cdir, b['path'])
                if not os.path.isfile(bpath):
                    bpath = os.path.join(cdir, 'src', b['path'])
                    if os.path.isfile(bpath):
                        b['path'] = os.path.join('src', b['path'])
                    else:
                        raise RuntimeError('could not find %s to build in %s', (build, cdir))


            d = cfg.get('dependencies', {})
            deps = []
            for k,v in d.iteritems():
                if type(v) is not dict:
                    deps.append({'name':k, 'req': v})
                elif v.has_key('path'):
                    req = v.get('version', '0')
                    deps.append({'name':k, 'path': v['path'], 'req':req})
            return (name, ver, deps, build)
    except Exception, e:
        print 'failed to load toml file for: %s' % cdir

    return (None, None, [], 'lib.rs')

def crate_info_from_index(idir, name, svr):
    if len(name) == 1:
        ipath = os.path.join(idir, '1', name)
    elif len(name) == 2:
        ipath = os.path.join(idir, '2', name)
    elif len(name) == 3:
        ipath = os.path.join(idir, '3', name[0:1], name)
    else:
        ipath = os.path.join(idir, name[0:2], name[2:4], name)

    print 'opening crate info: %s' % ipath
    dep_infos = []
    with open(ipath, 'rb') as fin:
        lines = fin.readlines()
        for l in lines:
            dep_infos.append(json.loads(l))

    passed = {}
    for info in dep_infos:
        if not info.has_key('vers'):
            continue
        sv = Semver(info['vers'])
        if svr.compare(sv):
            passed[sv] = info

    keys = sorted(passed.iterkeys())
    #print 'matched versions in sorted order:'
    #for k in keys:
    #    print '\t%s' % k

    best_match = keys.pop()
    print 'best match is %s-%s' % (name, best_match)
    best_info = passed[best_match]
    name = best_info.get('name', None)
    ver = best_info.get('vers', None)
    deps = best_info.get('deps', [])
    ftrs = best_info.get('features', [])
    cksum = best_info.get('cksum', None)
    return (name, ver, deps, ftrs, cksum)

def args_parser():
    parser = argparse.ArgumentParser(description='Cargo Bootstrap Tool')
    parser.add_argument('--cargo-root', type=str,  default=os.getcwd(),
                        help="specify the cargo repo root path")
    parser.add_argument('--target-dir', type=str, default=tempfile.mkdtemp(),
                        help="specify the path for storing built dependency libs")
    parser.add_argument('--crate-index', type=str, default=None,
                        help="path to where the crate index should be cloned")
    parser.add_argument('--test-semver', action='store_true',
                        help="run semver parsing tests")
    parser.add_argument('--no-clone', action='store_true',
                        help="skip cloning crates index, --target-dir must point to an existing clone of the crates index")
    parser.add_argument('--no-clean', action='store_true',
                        help="don't delete the target dir and crate index")
    return parser

def open_or_clone_repo(rdir, rurl, no_clone):
    try:
        repo = git.open_repo(rdir)
        return repo
    except:
        repo = None

    if repo is None and no_clone is False:
        print 'Cloning %s to %s' % (rurl, rdir)
        return git.clone(rurl, rdir)

    return repo

if __name__ == "__main__":
    try:
        # parse args
        parser = args_parser()
        args = parser.parse_args()

        if args.test_semver:
            test_semver()
            sys.exit(0)

        # clone the cargo index
        if args.crate_index is None:
            args.crate_index = os.path.normpath(os.path.join(args.target_dir, 'index'))
        print "cargo: %s, target: %s, index: %s" % \
              (args.cargo_root, args.target_dir, args.crate_index)

        index = open_or_clone_repo(args.crate_index, CRATES_INDEX, args.no_clone)
        cargo = open_or_clone_repo(args.cargo_root, CARGO_REPO, args.no_clone)

        if index is None:
            raise RuntimeError('You must have a local clone of the crates index ' \
                               'or omit --no-clone to allow this script to clone ' \
                               'it for you.')
        if cargo is None:
            raise RuntimeError('You must  have a local clone of the cargo repo '\
                               'so that this script can read the cargo toml file.')

        # load cargo deps
        name, ver, deps, build = crate_info_from_toml(args.cargo_root)
        cargo_crate = Crate(name, ver, deps, args.cargo_root, build)
        UNRESOLVED.append(cargo_crate)

        # resolve and download all of the dependencies
        print ''
        print '===================================='
        print '===== DOWNLOADING DEPENDENCIES ====='
        print '===================================='
        print ''
        while len(UNRESOLVED) > 0:
            crate = UNRESOLVED.pop(0)
            crate.resolve(args.target_dir, args.crate_index)

        # build cargo
        print ''
        print '=========================='
        print '===== BUILDING CARGO ====='
        print '=========================='
        print ''
        cargo_crate.build('bootstrap.py', args.target_dir)

        # cleanup
        if not args.no_clean:
            print "cleaning up..."
            shutil.rmtree(args.target_dir)
        print "done"

    except Exception, e:
        frame = inspect.trace()[-1]
        print >> sys.stderr, "\nException:\n from %s, line %d:\n %s\n" % (frame[1], frame[2], e)
        parser.print_help()
        if not args.no_clean:
            shutil.rmtree(args.target_dir)
        sys.exit(1)


