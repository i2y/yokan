#!/bin/zsh
# Every Wakakusa demo through both runs — CRuby over pixie's C ABI, and
# the spinel-compiled binary linking the same library — failing on any
# difference. Run it before any commit that touches the door, the ABI or
# a demo.
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

# The vocabulary is one table, and three files are written from it. If
# any of them is behind, the Ruby an app calls and the engine's idea of
# what it called have already parted.
if ! ruby tools/gen.rb --check; then
  echo "FAIL vocabulary (the generated files are behind elements.toml)"
  exit 1
fi
echo "OK   vocabulary"

# Scripted gates, with the same steps Yokan's sweep drives its own
# copy of each demo with, wherever the verbs exist here.
gate counter ./bin/wakakusa gate demo/counter.rb --script "click:+1,dump,input:Momo\, again"
gate forms   ./bin/wakakusa gate demo/forms.rb --script "click:Dark mode,slide:7,select:banana"
gate layout  ./bin/wakakusa gate demo/layout.rb --script "click:ping"
gate labels  ./bin/wakakusa gate demo/labels.rb --script "dump,click:save,dump"
gate badges  ./bin/wakakusa gate demo/badges.rb --script "click:flip,dump,click:flip"
gate charts  ./bin/wakakusa gate demo/charts.rb --script "click:next month,dump,click:next month"
gate quantities ./bin/wakakusa gate demo/quantities.rb --script "input@0:3,input@1:2.5,dump,input@0:abc,dump,input@0:500"
gate table   ./bin/wakakusa gate demo/table.rb --script "click:refresh,dump,click:refresh"
gate todo    ./bin/wakakusa gate demo/todo.rb --script "input:eggs,submit,dump,click:done,dump"
gate mixer   ./bin/wakakusa gate demo/mixer.rb --script "click:+1,click:mute,dump,input:live set,dump"
gate trend   ./bin/wakakusa gate demo/trend.rb --script "click:stir,dump"
gate edges   ./bin/wakakusa gate demo/edges.rb --script "click:oob,dump,click:grow,click:grow,click:partial,dump"
gate points  ./bin/wakakusa gate demo/points.rb --script "click:right,click:measure,dump,click:swap,dump"
gate cards   ./bin/wakakusa gate demo/cards.rb --script "click:+1,click:+10,dump"
gate styled  ./bin/wakakusa gate demo/styled.rb --script "click:+1,dump,click:flip,dump"

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
