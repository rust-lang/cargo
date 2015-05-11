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

import argparse, inspect, os, re, shutil, sys, tempfile
import pytoml as toml
import dulwich.porcelain as git

CRATES_INDEX = 'git://github.com/rust-lang/crates.io-index.git'
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

    def __str__(self):
        return self._input

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


CRATES = {}

class Crate(object):

    def __init__(self, crate, ver):
        self._crate = str(crate)
        self._ver = Semver(ver)

    def name(self):
        return self._crate

    def version(self):
        return self._version

    def __str__(self):
        return '%s-%s' % (self._crate, self._version)

    def __repr__(self):
        return self.__str__()

    def add_dep(self, crate):
        self._deps.append(crate.name(), create.version())


def crate_from_toml(cdir, idir):
    with open(os.path.join(cdir, 'Cargo.toml'), 'rb') as ctoml:
        cfg = toml.load(ctoml)
        crate = Crate(cfg['project']['name'], cfg['project']['version'])
        CRATES[crate] = crate
        import pdb; pdb.set_trace()
        process_toml_deps(crate, cfg['dependencies'], cdir, idir)

def create_from_index(cdir, idir):
    pass

def process_toml_deps(crate, deps, cdir, idir):
    pass
    #for k,v in deps.iteritems():
    #    if not deps.has_key(str(k)):
    #        deps[str(k)] = CargoDep(k, v)
    #return deps


def process_index_deps(crate, cdir, idir):
    pass

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

def clone_index(tdir):
    print "cloning crates index to: %s" % tdir
    repo = git.clone(CRATES_INDEX, repo_path)

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
        if not args.no_clone:
            clone_index(args.crate_index)
            print "\n"

        # load cargo deps
        #crate_from_toml(args.cargo_root, args.crate_index)

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


