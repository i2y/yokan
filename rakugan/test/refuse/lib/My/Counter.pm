# A module of the app's own, here so the fixture beside it reaches the
# translator: perl reads the file first, and a module it cannot find is
# perl's verdict, not the dialect's.
package My::Counter;
use strict;
use warnings;

sub next { return 1 }

1;
