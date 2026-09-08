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

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
