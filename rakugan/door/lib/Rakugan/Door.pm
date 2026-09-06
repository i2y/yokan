package Rakugan::Door;
# pixie's C face, one Perl sub per entry point. The C is in Door.xs
# beside this file; bin/rakugan builds the two into a bundle for the
# perl the app runs under, outside the tree.
use strict;
use warnings;
our $VERSION = '0.1.0';
require XSLoader;
XSLoader::load('Rakugan::Door', $VERSION);
1;
