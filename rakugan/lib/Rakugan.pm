package Rakugan;
# `use Rakugan;` is the one line an app needs, and it needs it twice: at
# the top of the file, and inside its class.
#
# At the top it turns on what the dialect assumes, the way `use v5.40`
# would (strict, warnings, signatures, say, try), plus utf8 and the
# class feature without its experimental warning, and it brings `run`.
# Inside the class it brings what the class writes with: the elements,
# `empty`, and the type names. A Perl import is per package, and a
# field's initializer runs at `new`, before `run` could lend anything —
# which is why the line is written twice rather than once.
use v5.40;
# The pragmas `import` turns on for the caller; `use v5.40` enables
# them for this file without loading the modules.
require feature;
require strict;
require warnings;
require utf8;
require builtin;
use Rakugan::Runtime;
use Rakugan::Elements;

our $VERSION = '0.1.0';

# --- the types, as values ---------------------------------------------------
#
# Types::Standard's spelling, so a type reads the way Perl already
# writes one: `Int`, `Str`, `ArrayRef[Int]`. At run time they are inert
# strings — nothing here checks anything; the translator reads them
# from the source and the compiled run holds the app to them.
# Constants, so `isa => Int, default => 0` in a list cannot swallow what
# follows; written as attributes because signatures are on in here.
sub Int  :prototype() { 'Int' }
sub Str  :prototype() { 'Str' }
sub Num  :prototype() { 'Num' }
sub Bool :prototype() { 'Bool' }
# `ArrayRef[Int]` parses as ArrayRef([Int]) because of the prototype.
sub ArrayRef :prototype(;$) { 'ArrayRef' . (@_ ? "[$_[0][0]]" : '') }
sub HashRef  :prototype(;$) { 'HashRef'  . (@_ ? "[$_[0][0]]" : '') }

# A container that starts empty says what it will hold:
#
#     field @items  = empty(Str);
#     field %scores = empty(Int);
#     my    @lines  = empty(Str);
#
# At run time this answers an empty list, which is what the field or
# the variable starts as. The translator reads the element type from
# the argument, so "a type is read from the initializer" has no
# exception for an empty one.
sub empty { return }

my @VOCAB = (@Rakugan::Elements::ELEMENTS, qw(run every task empty Int Str Num Bool ArrayRef HashRef));

sub import {
    my $class = shift;
    # The caller's lexical scope.
    strict->import;
    warnings->import;
    utf8->import;
    feature->import(':5.40');
    feature->import('class');
    warnings->unimport('experimental::class');
    builtin->import(':5.40');
    # The caller's package.
    my $pkg = caller;
    no strict 'refs';
    # `method add :Sig(Int => Str) ($n)`. perl hands an attribute to the
    # class the sub was compiled into and expects to be told which ones
    # it did not understand; `Sig` is understood and does nothing at run
    # time. The types are read out of the source by the translator, and
    # the compiled run is what holds the app to them.
    *{"${pkg}::MODIFY_CODE_ATTRIBUTES"} = sub {
        my ($class, $code, @attrs) = @_;
        return grep { !/\ASig\b/ } @attrs;
    };
    for my $name (@VOCAB) {
        my $from = defined &{"Rakugan::Elements::$name"} ? "Rakugan::Elements::$name"
                 : $name =~ /\A(?:run|every|task)\z/       ? "Rakugan::Runtime::$name"
                 :                                            "Rakugan::$name";
        *{"${pkg}::$name"} = \&{$from};
    }
}

1;
