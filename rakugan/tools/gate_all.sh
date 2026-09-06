#!/bin/zsh
# Every Rakugan demo through both runs — perl over pixie's C ABI, and
# the binary pixie built from the translated .pix — failing on any
# difference. Run it before any commit that touches the door, the
# translator, the generator or a demo.
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

# The vocabulary is one table, and two files here are written from it.
# If either is behind, the Perl an app calls and the .pix the translator
# writes have already parted from what the engine reads.
if ! perl tools/gen.pl --check; then
  echo "FAIL vocabulary (the generated files are behind crates/pixie-capi/elements.toml)"
  exit 1
fi
echo "OK   vocabulary"

# Scripted gates, with the same steps Yokan's and Wakakusa's sweeps
# drive their copies of each demo with.
gate counter ./bin/rakugan gate demo/counter.pl --script "click:+1,dump,input:Momo\, again"
gate layout  ./bin/rakugan gate demo/layout.pl --script "click:ping"
gate labels  ./bin/rakugan gate demo/labels.pl --script "dump,click:save,dump"
gate badges  ./bin/rakugan gate demo/badges.pl --script "click:flip,dump,click:flip"

echo "SWEEP DONE: pass=$pass fail=$fail failed:$failed"
[ "$fail" -eq 0 ]
