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

# The doors have to be the same Ruby above the ABI, or the two runs are
# not running the same program and nothing below proves anything.
share() { awk '/both doors share/{f=1} /^# --- the run/{f=0} f' "$1"; }
if ! diff -q <(share door/cruby/wakakusa.rb) <(share door/spinel/wakakusa.rb) >/dev/null; then
  echo "FAIL doors (the shared half differs between door/cruby and door/spinel)"
  diff <(share door/cruby/wakakusa.rb) <(share door/spinel/wakakusa.rb)
  exit 1
fi
echo "OK   doors"

gate counter ./bin/wakakusa gate demo/counter.rb --script "click:+1,click:+10,dump"

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
