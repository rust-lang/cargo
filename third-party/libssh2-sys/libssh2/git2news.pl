#!/usr/bin/env perl
# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause

# git log --pretty=fuller --no-color --date=short --decorate=full

my @mname = ('Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
             'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec' );

sub nicedate {
    my ($date)=$_;

    if($date =~ /(\d\d\d\d)-(\d\d)-(\d\d)/) {
        return sprintf("%d %s %4d", $3, $mname[$2-1], $1);
    }
    return $date;
}

print
'   Changelog for the libssh2 project. Generated with git2news.pl
';

my $line;
my $tag;
while(<STDIN>) {
    my $l = $_;

    if($l =~/^commit ([[:xdigit:]]*) ?(.*)/) {
        $co = $1;
        my $ref = $2;
        if ($ref =~ /refs\/tags\/(libssh2-|VERSION\.)([0-9._]*)/) {
            $tag = $2;
        } else {
            $tag = '';
        }
    }
    elsif($l =~ /^Author: *(.*) +</) {
        $a = $1;
    }
    elsif($l =~ /^Commit: *(.*) +</) {
        $c = $1;
    }
    elsif($l =~ /^CommitDate: (.*)/) {
        $date = nicedate($1);
    }
    elsif($l =~ /^(    )(.*)/) {
        my $extra;
        if ($tag) {
            # Version entries have a special format
            print "\nVersion " . $tag." ($date)\n";
            $oldc = "";
            $tag = "";
        }
        if($a ne $c) {
            $extra=sprintf("\n- [%s brought this change]\n\n  ", $a);
        }
        else {
            $extra="\n- ";
        }
        if($co ne $oldco) {
            if($c ne $oldc) {
                print "\n$c ($date)$extra";
            }
            else {
                print "$extra";
            }
            $line =0;
        }

        $oldco = $co;
        $oldc = $c;
        $olddate = $date;
        if($line++) {
            print "  ";
        }
        print $2."\n";
    }
}
