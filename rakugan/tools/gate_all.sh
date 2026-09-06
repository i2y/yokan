#!/bin/zsh
# Every Rakugan demo through both runs — perl over pixie's C ABI, and
# the binary pixie built from the translated .pix — failing on any
# difference. Run it before any commit that touches the door, the
# translator or a demo.
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

# Scripted gates, with the same steps Yokan's and Wakakusa's sweeps
# drive their copies of each demo with.
gate counter ./bin/rakugan gate demo/counter.pl --script "click:+1,dump,input:Momo\, again"

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
