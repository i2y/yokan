#!/usr/bin/env perl
# Every complete app in the tour, run.
#
# A fenced `perl` block that uses Rakugan and ends in `run(` is an app,
# not an illustration, so it is written out and put through the same
# command a demo goes through. A `<!-- script: … -->` line just above
# the fence says what to drive it with; without one the block is only
# checked, which is what an example with no state to change needs.
#
# They are written into `demo/.gate/`, beside every other generated
# thing. The point is that the tour cannot drift: a rename in the
# vocabulary or a new refusal breaks the page that teaches it, here,
# before a reader meets it.
use strict;
use warnings;
use File::Basename qw(basename dirname);
use File::Path qw(make_path);
use File::Spec;

my $ROOT = File::Spec->rel2abs(File::Spec->catdir(dirname(__FILE__), File::Spec->updir));
my $OUT  = File::Spec->catdir($ROOT, 'demo', '.gate');

sub apps {
    my ($path) = @_;
    open my $fh, '<:encoding(UTF-8)', $path or die "$path: $!\n";
    my (@found, $script, $fence);
    while (my $line = <$fh>) {
        if (defined $fence) {
            if ($line =~ /\A```/) {
                push @found, [$script, join('', @$fence)] if join('', @$fence) =~ /run\(/;
                ($fence, $script) = (undef, undef);
            } else {
                push @$fence, $line;
            }
            next;
        }
        if ($line =~ /<!-- script:\s*(.*?)\s*-->/) {
            $script = $1;
        } elsif ($line =~ /\A```perl/) {
            $fence = [];
        } elsif ($line =~ /\S/) {
            $script = undef;
        }
    }
    close $fh;
    return grep { $_->[1] =~ /^use Rakugan;/m } @found;
}

my @pages = @ARGV ? @ARGV : (File::Spec->catfile($ROOT, 'TOUR.md'));
make_path($OUT);
my $failed = 0;
for my $page (@pages) {
    my @list = apps($page);
    warn basename($page) . ": no complete app in it\n" unless @list;
    for my $i (0 .. $#list) {
        my ($script, $body) = @{ $list[$i] };
        # The whole path names the file, so a page under website/ cannot
        # overwrite the tour page of the same name.
        (my $stem = $page) =~ s{\A\./}{};
        $stem =~ s/\.md\z//;
        $stem = lc $stem;
        $stem =~ tr{/.-}{___};
        my $file = File::Spec->catfile($OUT, sprintf('%s_%02d.pl', $stem, $i));
        open my $out, '>:encoding(UTF-8)', $file or die "$file: $!\n";
        print {$out} $body;
        close $out;
        my @cmd = (File::Spec->catfile($ROOT, 'bin', 'rakugan'));
        push @cmd, $script ? ('gate', $file, '--script', $script) : ('check', $file);
        my $ok = system("@{[ join ' ', map { qq('$_') } @cmd ]} >/dev/null 2>&1") == 0;
        printf "%-4s %s%s\n", $ok ? 'OK' : 'FAIL', basename($file),
            $script ? "  ($script)" : '';
        $failed++ unless $ok;
    }
}
print $failed ? "TOUR: $failed failed\n" : "TOUR: every example runs\n";
exit($failed ? 1 : 0);
