#!/usr/bin/env bash
# Every refusal has a fixture that triggers it and the message it must
# print, word for word. A rule whose message drifts is a rule that
# stopped teaching.
# `env bash`, not a login shell's own: nothing here is zsh's, and
# bash is the one interpreter both supported platforms have.
cd "$(dirname "$0")/.." || exit 1
pass=0; fail=0
for f in test/refuse/*.rb; do
  want="${f%.rb}.expected"
  got=$(./bin/wakakusa check "$f" 2>&1)
  if [ "$got" = "$(cat "$want")" ]; then
    pass=$((pass+1))
  else
    fail=$((fail+1))
    echo "FAIL $(basename "$f")"
    diff <(cat "$want") <(echo "$got")
  fi
done
echo "REFUSALS: pass=$pass fail=$fail"
[ "$fail" -eq 0 ]
