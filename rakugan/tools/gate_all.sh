#!/bin/zsh
# Every Rakugan demo through both runs — perl over pixie's C ABI, and
# the binary pixie built from the translated .pix — failing on any
# difference. Run it before any commit that touches the door, the
# translator, the generator or a demo.
cd "$(dirname "$0")/.." || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/pixie/target}"
pass=0; fail=0; failed=""
gate() {
  name="$1"; shift
  if "$@" 2>&1 | grep -q "GATE OK"; then
    pass=$((pass+1)); echo "OK   $name"
  else
    fail=$((fail+1)); failed="$failed $name"; echo "FAIL $name"
  fi
}

# The vocabulary is one table, and two files here are written from it.
# If either is behind, the Perl an app calls and the .pix the translator
# writes have already parted from what the engine reads.
if ! perl tools/gen.pl --check; then
  echo "FAIL vocabulary (the generated files are behind crates/pixie-capi/elements.toml)"
  exit 1
fi
echo "OK   vocabulary"

# Every refusal that stands for a decision has a file that triggers it
# and the message it must print. A rule that stops firing is a promise
# the dialect quietly dropped.
if ! ./tools/refuse_test.sh > /dev/null; then
  echo "FAIL refusals"
  ./tools/refuse_test.sh
  exit 1
fi
echo "OK   refusals"

# The twins of Perl's own answers are held to what perl printed. A
# table that is not what perl says NOW is a twin that has drifted, or a
# perl that has moved.
PERL_FOR_TABLES="$(./tools/perl_setup.sh --path)/bin/perl"
[ -x "$PERL_FOR_TABLES" ] || PERL_FOR_TABLES="${RAKUGAN_PERL:-perl}"
if ! "$PERL_FOR_TABLES" tools/gen_expected.pl --check; then
  echo "FAIL tables (crates/rakugan-stdlib/tests/expected is behind what perl prints)"
  exit 1
fi
if ! (cd .. && cargo test -q -p rakugan-stdlib > /dev/null 2>&1); then
  echo "FAIL twins (a twin does not answer what perl printed)"
  (cd .. && cargo test -q -p rakugan-stdlib 2>&1 | tail -20)
  exit 1
fi
echo "OK   twins"

# Scripted gates, with the same steps Yokan's and Wakakusa's sweeps
# drive their copies of each demo with.
gate counter ./bin/rakugan gate demo/counter.pl --script "click:+1,dump,input:Momo\, again"
gate layout  ./bin/rakugan gate demo/layout.pl --script "click:ping"
gate labels  ./bin/rakugan gate demo/labels.pl --script "dump,click:save,dump"
gate badges  ./bin/rakugan gate demo/badges.pl --script "click:flip,dump,click:flip"
gate styled  ./bin/rakugan gate demo/styled.pl --script "click:+1,dump,click:flip,dump"
gate mixer   ./bin/rakugan gate demo/mixer.pl --script "click:+1,click:mute,dump,input:live set,dump"
gate panels  ./bin/rakugan gate demo/panels.pl --script "select:stack,dump,click:about,dump,click:close"
gate control ./bin/rakugan gate demo/control.pl --script "click:pick 1,dump,click:hint,click:tab 2,dump"
gate cards   ./bin/rakugan gate demo/cards.pl --script "click:+1,click:+10,dump"
gate todo    ./bin/rakugan gate demo/todo.pl --script "input:eggs,submit,dump,click:done,dump"
gate filter  ./bin/rakugan gate demo/filter.pl --script "select:crit,dump,select:all,dump"
gate loading ./bin/rakugan gate demo/loading.pl --script "click:step,click:step,dump,click:busy"
gate trend   ./bin/rakugan gate demo/trend.pl --script "click:add point,dump,click:raise limit,dump"
gate quantities ./bin/rakugan gate demo/quantities.pl --script "input@0:3,input@1:2.5,dump,input@0:abc,dump,input@0:500"
gate shared  ./bin/rakugan gate demo/shared.pl --script "click:lock,click:save,input:typed,dump,click:lock,click:save,dump"
gate lookup  ./bin/rakugan gate demo/lookup.pl --script "click:apple,dump,click:cherry,dump,click:miss,dump"
gate table   ./bin/rakugan gate demo/table.pl --script "click:refresh,dump,click:refresh"
gate charts  ./bin/rakugan gate demo/charts.pl --script "click:next month,dump,click:next month"
gate stdlib  ./bin/rakugan gate demo/stdlib.pl --script "click:measure,click:stats,click:sift,click:combine,dump,click:stamp,click:words,click:set,click:first,click:scan,click:tidy,dump"
gate flow    ./bin/rakugan gate demo/flow.pl --script "click:step,click:tally,dump,click:bump3,click:find,dump"
gate forms   ./bin/rakugan gate demo/forms.pl --script "click:Dark mode,slide:7,select:banana"
gate roster  ./bin/rakugan gate demo/roster.pl --script "select:member 7,dump,click:score,dump,click:score,dump"
gate points  ./bin/rakugan gate demo/points.pl --script "click:right,click:measure,dump,click:swap,dump"
gate edges   ./bin/rakugan gate demo/edges.pl --script "click:oob,dump,click:shrink,click:shrink,click:partial,dump"
gate calc    ./bin/rakugan gate demo/calc.pl --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate calcgrid ./bin/rakugan gate demo/calcgrid.pl --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate tasks   ./bin/rakugan gate demo/tasks.pl --script "click:start slow work,dump,click:start slow work,dump"
gate dashboard ./bin/rakugan gate demo/dashboard.pl --script "advance:1000,advance:1000,dump"

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
