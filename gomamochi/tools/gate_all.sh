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
if ! go vet . ./cmd/... ./internal/... ./tools/... ; then
  echo "FAIL vet"
  exit 1
fi
echo "OK   vet"

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

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
