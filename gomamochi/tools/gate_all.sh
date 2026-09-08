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

# The package, the command and the door, as Go's own checker reads
# them. The demos are not a package: each is its own `main`.
if ! go vet . ./cmd/... ./internal/... ; then
  echo "FAIL vet"
  exit 1
fi
echo "OK   vet"

# Scripted gates, with the same steps the other languages' sweeps drive
# their copies of each demo with.
gate counter ./bin/gomamochi gate demo/counter.go --script "click:+1,dump,input:Momo\, again"
gate todo    ./bin/gomamochi gate demo/todo.go --script "input:eggs,submit,dump,click:done,dump"
gate tasks   ./bin/gomamochi gate demo/tasks.go --script "click:start slow work,dump,click:start slow work,dump"

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
