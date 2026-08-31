#!/bin/zsh
# Gate every demo through both runs (interpreted CPython and the
# compiled binary) and fail on any dump difference. Run from
# crates/yokan. Four demos are development-only BY DESIGN (dict
# state — the honest-list item) and are listed, not gated.
cd "$(dirname "$0")/.." || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/pixie/target}"
pass=0; fail=0; failed=""
gate() {
  local name="$1"; shift
  if "$@" 2>&1 | grep -q "GATE OK"; then
    pass=$((pass+1)); echo "OK   $name"
  else
    fail=$((fail+1)); failed="$failed $name"; echo "FAIL $name"
  fi
}
# Scripted gates (interaction coverage beyond the startup dump).
gate counter python3 yokan_gate.py gate demo/counter.py --script "click:+1,input:Momo"
gate forms   python3 yokan_gate.py gate demo/forms.py --script "click:Dark mode,slide:7,select:banana"
gate calc    python3 yokan_gate.py gate demo/calc.py --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate calcgrid python3 yokan_gate.py gate demo/calcgrid.py --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate links   python3 yokan_gate.py gate demo/links.py --script "click:build,click:peek,click:drop,click:peek"
# Fixture- and dependency-carrying gates.
gate rustcrate python3 yokan_gate.py gate demo/rustcrate.py --script "click:run"
# Fixture- and dependency-carrying gates.
gate dbnotes python3 yokan_gate.py gate demo/dbnotes.py --fresh demo/.gate/notes.db
gate pystats env -u VIRTUAL_ENV uv run --quiet --with numpy python3 yokan_gate.py gate demo/pystats.py
gate proj     python3 yokan_gate.py gate demo/proj/app.py --script "click:run"
# Multi-module apps.
gate multi    python3 yokan_gate.py gate demo/multi/app.py
gate opsboard python3 yokan_gate.py gate demo/opsboard/app.py
# Everything else: startup-dump gates.
for f in demo/*.py; do
  b=$(basename "$f" .py)
  case "$b" in
    counter|forms|links|calc|calcgrid|dbnotes|pystats|rustcrate) continue;;
    app|csv_viewer|dashboard|tasks)
      echo "SKIP $b (development-only by design: dict state)"; continue;;
  esac
  gate "$b" python3 yokan_gate.py gate "$f"
done
echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
