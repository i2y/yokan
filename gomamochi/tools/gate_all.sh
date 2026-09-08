#!/usr/bin/env bash
# Every Gomamochi demo through both runs — the file under yaegi through
# the door, and the binary gc built from it, both opening pixie's C face
# — failing on any difference. Run it before any commit that touches the
# door, the elements or a demo.
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

# The vocabulary is one table, and two files are written from it. If
# either is behind, the Go an app calls and the engine's idea of what
# it called have already parted.
if ! go run ./tools/gen --check; then
  echo "FAIL vocabulary (the generated files are behind elements.toml)"
  exit 1
fi
echo "OK   vocabulary"

# The package, the command, the door and the generator, as Go's own
# checker reads them. The demos are not a package: each is its own
# `main`.
if ! go vet . ./cmd/... ./internal/... ./tools/... ./website/... ; then
  echo "FAIL vet"
  exit 1
fi
echo "OK   vet"

# The command's own tests: the shapes a package takes on the platform
# this machine is not, checked for their layout here.
if ! go test ./cmd/... > /dev/null 2>&1; then
  echo "FAIL tests"
  go test ./cmd/...
  exit 1
fi
echo "OK   tests"

# Every refusal has a fixture and the message it must print.
if ! ./tools/refuse_test.sh > /dev/null; then
  echo "FAIL refusals"
  ./tools/refuse_test.sh
  exit 1
fi
echo "OK   refusals"

# Scripted gates, with the same steps the other languages' sweeps drive
# their copies of each demo with.
gate counter ./bin/gomamochi gate demo/counter.go --script "click:+1,dump,input:Momo\, again"
# The same screen written the other way: the package under a name.
gate prefixed ./bin/gomamochi gate demo/prefixed.go --script "click:+1,dump,input:Momo\, again"
gate badges  ./bin/gomamochi gate demo/badges.go --script "click:flip,dump,click:flip"
gate labels  ./bin/gomamochi gate demo/labels.go --script "dump,click:save,dump"
gate layout  ./bin/gomamochi gate demo/layout.go --script "click:ping"
gate forms   ./bin/gomamochi gate demo/forms.go --script "click:Dark mode,slide:7,select:banana"
gate cards   ./bin/gomamochi gate demo/cards.go --script "click:+1,click:+10,dump"
gate todo    ./bin/gomamochi gate demo/todo.go --script "input:eggs,submit,dump,click:done,dump"
gate tasks   ./bin/gomamochi gate demo/tasks.go --script "click:start slow work,dump,click:start slow work,dump"
gate calc    ./bin/gomamochi gate demo/calc.go --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate roster  ./bin/gomamochi gate demo/roster.go --script "select:member 7,dump,click:score,dump,click:score,dump"
gate dashboard ./bin/gomamochi gate demo/dashboard.go --script "advance:1000,advance:1000,dump"
# Frames, keys and a canvas: a script's `advance:` ticks the timer and
# `keydown:` holds a key for the frames between it and `keyup:`.
gate canvas  ./bin/gomamochi gate demo/canvas.go --script "advance:50,dump,keydown:left,advance:50,advance:50,dump,keyup:left,keydown:space,advance:50,keyup:space,dump"
gate jump    ./bin/gomamochi gate demo/jump.go --script "advance:34,advance:34,dump,keydown:right,advance:34,advance:34,advance:34,dump,keyup:right,advance:34,dump"
gate shooter ./bin/gomamochi gate demo/shooter.go --script "advance:34,advance:34,dump,keydown:enter,advance:34,advance:34,keyup:enter,advance:34,dump,keydown:space,advance:34,advance:34,keyup:space,advance:34,advance:34,dump"


# The rest of the vocabulary and the app's shape, in the order the
# other languages' sweeps drive them.
gate control ./bin/gomamochi gate demo/control.go --script "click:pick 1,dump,click:hint,click:tab 2,dump"
gate charts  ./bin/gomamochi gate demo/charts.go --script "click:next month,dump,click:next month"
gate quantities ./bin/gomamochi gate demo/quantities.go --script "input@0:3,input@1:2.5,dump,input@0:abc,dump,input@0:500"
gate table   ./bin/gomamochi gate demo/table.go --script "click:refresh,dump,click:refresh"
gate mixer   ./bin/gomamochi gate demo/mixer.go --script "click:+1,click:mute,dump,input:live set,dump"
gate trend   ./bin/gomamochi gate demo/trend.go --script "click:add point,dump,click:raise limit,dump"
gate edges   ./bin/gomamochi gate demo/edges.go --script "click:oob,dump,click:grow,click:grow,click:partial,dump"
gate points  ./bin/gomamochi gate demo/points.go --script "click:right,click:measure,dump,click:swap,dump"
gate styled  ./bin/gomamochi gate demo/styled.go --script "click:+1,dump,click:flip,dump"
gate panels  ./bin/gomamochi gate demo/panels.go --script "select:stack,dump,click:about,dump,click:close"
gate reader  ./bin/gomamochi gate demo/reader.go --script "click:fetch,dump"
gate csv_viewer ./bin/gomamochi gate demo/csv_viewer.go --script "input:momo,dump,input:zzz,dump"
# The two that keep a database: each run starts from the same nothing.
gate dbnotes ./bin/gomamochi gate demo/dbnotes.go --fresh demo/.gate/notes.db --script "click:setup,click:load,dump"
# The ledger types a name with an apostrophe, which only a BOUND value survives.
gate ledger  ./bin/gomamochi gate demo/ledger.go --fresh demo/.gate/ledger.db --script "click:reset,input@0:o'brien,input@1:250,click:food,dump"
gate stdlib  ./bin/gomamochi gate demo/stdlib.go --script "click:measure,click:stats,click:sift,click:count,click:combine,click:stamp,click:parse,click:csv,click:words,click:set,dump,click:write,click:scan,dump"
gate about   ./bin/gomamochi gate demo/about.go --script "click:copy link,dump,click:Website"
# Sound: silent under a script, so what the two runs compare is the screen.
gate sound   ./bin/gomamochi gate demo/sound.go --script "click:jump,dump,slide:0.3,click:blast,click:stop,dump"
gate keys    ./bin/gomamochi gate demo/keys.go --script "click:+1,click:+1,key:cmd+s,dump,key:x,menu:Clear,dump,key:cmd+shift+c,key:cmd+shift+v,dump"
# The picker needs something on disk to choose and to drop.
mkdir -p demo/.gate && echo "a file the picker can read" > demo/.gate/fs_probe.txt
gate picker  ./bin/gomamochi gate demo/picker.go --script "file:demo/.gate/fs_probe.txt,click:open…,dump,drop:demo/.gate/fs_probe.txt,dump"
gate flow    ./bin/gomamochi gate demo/flow.go --script "click:step,click:tally,dump,click:bump3,click:find,dump"
gate moods   ./bin/gomamochi gate demo/moods.go --script "click:flip,click:pick,click:describe,dump,click:track,click:clear,click:describe,dump,click:wipe,dump"
gate calcgrid ./bin/gomamochi gate demo/calcgrid.go --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate links   ./bin/gomamochi gate demo/links.go --script "click:build,click:peek,dump,click:drop,click:peek,dump"
gate files   ./bin/gomamochi gate demo/files.go --script "click:save,click:append,click:load,click:list,dump,click:data dir,dump,click:remove,dump"
gate lookup  ./bin/gomamochi gate demo/lookup.go --script "click:apple,dump,click:cherry,dump,click:miss,dump"
gate dialog  ./bin/gomamochi gate demo/dialog.go --script "click:open dialog,dump,click:accept,dump"
gate loading ./bin/gomamochi gate demo/loading.go --script "click:step,click:step,dump,click:busy"
gate filter  ./bin/gomamochi gate demo/filter.go --script "select:crit,dump,select:all,dump"
gate shared  ./bin/gomamochi gate demo/shared.go --script "click:lock,click:save,input:typed,dump,click:lock,click:save,dump"

# The tour teaches the vocabulary, so it has to hold to it: every
# complete app in either language, through the same command.
if go run ./tools/tourcheck TOUR.md TOUR.ja.md \
     website/docs/tour.md website/docs/tour-logic.md website/docs/tour-canvas.md \
     website/docs/tour-ui.md website/docs/tour-lib.md website/docs/tour-ship.md \
     website/docs-ja/tour.md website/docs-ja/tour-logic.md website/docs-ja/tour-canvas.md \
     website/docs-ja/tour-ui.md website/docs-ja/tour-lib.md website/docs-ja/tour-ship.md \
     > /dev/null 2>&1; then
  pass=$((pass + 1)); echo "OK   tour"
else
  fail=$((fail + 1)); failed="$failed tour"; echo "FAIL tour"
fi

# The site quotes what lives elsewhere: the tour, the vocabulary table,
# the list of demos, the refusals' own wording. Each has one source of
# truth, and the checker runs every writer with --check.
if go run ./website/tools/sitecheck > /dev/null 2>&1; then
  pass=$((pass + 1)); echo "OK   site"
else
  fail=$((fail + 1)); failed="$failed site"; echo "FAIL site"
  go run ./website/tools/sitecheck 2>&1 | grep FAIL
fi

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
