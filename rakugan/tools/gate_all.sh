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
gate flow    ./bin/rakugan gate demo/flow.pl --script "click:step,click:tally,dump,click:bump3,click:find,dump"

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
