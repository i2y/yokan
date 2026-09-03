#!/bin/zsh
# Gate every demo through both runs (interpreted CPython and the
# compiled binary) and fail on any dump difference. Run from
# crates/yokan. Two demos are development-only BY DESIGN (dict
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
gate counter python3 yokan_gate.py gate demo/counter.py --script "click:+1,dump,input:Momo\, again"
gate forms   python3 yokan_gate.py gate demo/forms.py --script "click:Dark mode,slide:7,select:banana"
gate postcard python3 yokan_gate.py gate demo/postcard.py --script "click:send"
gate calc    python3 yokan_gate.py gate demo/calc.py --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate calcgrid python3 yokan_gate.py gate demo/calcgrid.py --script "click:7,click:×,click:6,click:=,click:%,click:±,click:C,click:1,click:2,click:.,click:5,click:÷,click:4,click:="
gate links   python3 yokan_gate.py gate demo/links.py --script "click:build,click:peek,click:drop,click:peek"
gate table   python3 yokan_gate.py gate demo/table.py --script "click:refresh,dump,click:refresh"
gate tasks   python3 yokan_gate.py gate demo/tasks.py --script "click:start slow work,dump"
gate dashboard python3 yokan_gate.py gate demo/dashboard.py --script "advance:1000,advance:1000,dump"
gate keys    python3 yokan_gate.py gate demo/keys.py --script "click:+1,click:+1,key:cmd+s,dump,key:x,menu:Clear,dump,key:cmd+shift+c,key:cmd+shift+v,dump"
gate picker  python3 yokan_gate.py gate demo/picker.py --script "file:demo/.gate/fs_probe.txt,click:open…,dump,drop:demo/.gate/fs_probe.txt,dump"
# Wave 1 of the widget fleet (2026-09-03): each new element in its own demo.
gate layout  python3 yokan_gate.py gate demo/layout.py --script "click:ping"
gate about   python3 yokan_gate.py gate demo/about.py --script "click:copy link,dump,click:Website"
gate filter  python3 yokan_gate.py gate demo/filter.py --script "select:crit,dump,select:all"
gate loading python3 yokan_gate.py gate demo/loading.py --script "click:step,click:step,dump,click:busy"
gate labels  python3 yokan_gate.py gate demo/labels.py --script "dump,click:save,dump"
gate badges  python3 yokan_gate.py gate demo/badges.py --script "click:flip,dump,click:flip"
gate quantities python3 yokan_gate.py gate demo/quantities.py --script "input@0:3,input@1:2.5,dump,input@0:abc,dump,input@0:500"
gate charts  python3 yokan_gate.py gate demo/charts.py --script "click:next month,dump,click:next month"
gate roster  python3 yokan_gate.py gate demo/roster.py --script "select:member 7,dump,click:score,dump,click:score,dump,select@1:member 3,dump"
# The shared properties on every element; the middle steps are inert while locked.
gate shared  python3 yokan_gate.py gate demo/shared.py --script "click:lock,click:save,input:typed,dump,click:lock,click:save,dump"
gate stdlib  python3 yokan_gate.py gate demo/stdlib.py --script "click:measure,click:stats,click:roll,click:parse,click:stamp,click:write,dump,click:write list,dump"
gate files   python3 yokan_gate.py gate demo/files.py --script "click:save,click:append,click:load,click:list,dump,click:data dir,dump,click:remove,dump"
gate webfetch python3 yokan_gate.py gate demo/webfetch.py --script "click:start,click:fetch,dump,click:headers,dump,click:post,dump,click:status,dump"
# Fixture- and dependency-carrying gates.
gate rustcrate python3 yokan_gate.py gate demo/rustcrate.py --script "click:run"
# Fixture- and dependency-carrying gates.
gate dbnotes python3 yokan_gate.py gate demo/dbnotes.py --fresh demo/.gate/notes.db
# The ledger writes what the script types, so each tier starts from an
# empty database — and the name it types carries an apostrophe, which
# only a BOUND parameter survives.
gate ledger  python3 yokan_gate.py gate demo/ledger.py --fresh demo/.gate/ledger.db --script "click:reset,input@0:o'brien,input@1:250,click:food,dump"
gate pystats env -u VIRTUAL_ENV uv run --quiet --with numpy python3 yokan_gate.py gate demo/pystats.py
gate proj     python3 yokan_gate.py gate demo/proj/app.py --script "click:run"
# Multi-module apps.
gate multi    python3 yokan_gate.py gate demo/multi/app.py
gate opsboard python3 yokan_gate.py gate demo/opsboard/app.py
# Everything else: startup-dump gates.
for f in demo/*.py; do
  b=$(basename "$f" .py)
  case "$b" in
    counter|forms|links|calc|calcgrid|postcard|table|tasks|dashboard|dbnotes|pystats|rustcrate) continue;;
    stdlib|files|webfetch|ledger|keys|picker) continue;;
    layout|about|filter|loading|labels|badges|quantities|charts|roster) continue;;
    shared) continue;;
    app|csv_viewer)
      echo "SKIP $b (development-only by design: dict state)"; continue;;
  esac
  gate "$b" python3 yokan_gate.py gate "$f"
done
echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
