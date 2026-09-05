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

# Every refusal has a fixture and the message it must print.
if ! ./tools/refuse_test.sh > /dev/null; then
  echo "FAIL refusals"
  ./tools/refuse_test.sh
  exit 1
fi
echo "OK   refusals"

# Scripted gates, with the same steps Yokan's sweep drives its own
# copy of each demo with, wherever the verbs exist here.
gate counter ./bin/wakakusa gate demo/counter.rb --script "click:+1,dump,input:Momo\, again"
# The same screen written the other way: a container's children as a block.
gate blockform ./bin/wakakusa gate demo/blockform.rb --script "click:+1,dump,input:Momo\, again"
# Ordinary Ruby inside a view: if, unless, a ternary, a loop.
gate control ./bin/wakakusa gate demo/control.rb --script "click:pick 1,dump,click:hint,click:tab 2,dump"
gate forms   ./bin/wakakusa gate demo/forms.rb --script "click:Dark mode,slide:7,select:banana"
gate layout  ./bin/wakakusa gate demo/layout.rb --script "click:ping"
gate labels  ./bin/wakakusa gate demo/labels.rb --script "dump,click:save,dump"
gate badges  ./bin/wakakusa gate demo/badges.rb --script "click:flip,dump,click:flip"
gate charts  ./bin/wakakusa gate demo/charts.rb --script "click:next month,dump,click:next month"
gate quantities ./bin/wakakusa gate demo/quantities.rb --script "input@0:3,input@1:2.5,dump,input@0:abc,dump,input@0:500"
gate table   ./bin/wakakusa gate demo/table.rb --script "click:refresh,dump,click:refresh"
gate todo    ./bin/wakakusa gate demo/todo.rb --script "input:eggs,submit,dump,click:done,dump"
gate mixer   ./bin/wakakusa gate demo/mixer.rb --script "click:+1,click:mute,dump,input:live set,dump"
gate trend   ./bin/wakakusa gate demo/trend.rb --script "click:add point,dump,click:raise limit,dump"
gate edges   ./bin/wakakusa gate demo/edges.rb --script "click:oob,dump,click:grow,click:grow,click:partial,dump"
gate points  ./bin/wakakusa gate demo/points.rb --script "click:right,click:measure,dump,click:swap,dump"
gate cards   ./bin/wakakusa gate demo/cards.rb --script "click:+1,click:+10,dump"
gate styled  ./bin/wakakusa gate demo/styled.rb --script "click:+1,dump,click:flip,dump"
gate panels  ./bin/wakakusa gate demo/panels.rb --script "select:stack,dump,click:about,dump,click:close"
gate roster  ./bin/wakakusa gate demo/roster.rb --script "select:member 7,dump,click:score,dump,click:score,dump"
gate about   ./bin/wakakusa gate demo/about.rb --script "click:copy link,dump,click:Website"
gate keys    ./bin/wakakusa gate demo/keys.rb --script "click:+1,click:+1,key:cmd+s,dump,key:x,menu:Clear,dump,key:cmd+shift+c,key:cmd+shift+v,dump"
# The picker needs something on disk to choose and to drop.
mkdir -p demo/.gate && echo "a file the picker can read" > demo/.gate/fs_probe.txt
gate picker  ./bin/wakakusa gate demo/picker.rb --script "file:demo/.gate/fs_probe.txt,click:open…,dump,drop:demo/.gate/fs_probe.txt,dump"
gate flow    ./bin/wakakusa gate demo/flow.rb --script "click:step,click:tally,dump,click:bump3,click:find,dump"
gate moods   ./bin/wakakusa gate demo/moods.rb --script "click:flip,click:pick,click:describe,dump,click:track,click:clear,click:describe,dump,click:wipe,dump"
gate calc    ./bin/wakakusa gate demo/calc.rb --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate calcgrid ./bin/wakakusa gate demo/calcgrid.rb --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate links   ./bin/wakakusa gate demo/links.rb --script "click:build,click:peek,dump,click:drop,click:peek,dump"
gate files   ./bin/wakakusa gate demo/files.rb --script "click:save,click:append,click:load,click:list,dump,click:data dir,dump,click:remove,dump"
gate lookup  ./bin/wakakusa gate demo/lookup.rb --script "click:apple,dump,click:cherry,dump,click:miss,dump"
gate dialog  ./bin/wakakusa gate demo/dialog.rb --script "click:open dialog,dump,click:accept,dump"
gate tasks   ./bin/wakakusa gate demo/tasks.rb --script "click:start slow work,dump,click:start slow work,dump"
gate dashboard ./bin/wakakusa gate demo/dashboard.rb --script "advance:1000,advance:1000,dump"
# The shared properties on every element; the middle steps are inert while locked.
gate loading ./bin/wakakusa gate demo/loading.rb --script "click:step,click:step,dump,click:busy"
gate filter  ./bin/wakakusa gate demo/filter.rb --script "select:crit,dump,select:all,dump"
gate shared  ./bin/wakakusa gate demo/shared.rb --script "click:lock,click:save,input:typed,dump,click:lock,click:save,dump"

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
