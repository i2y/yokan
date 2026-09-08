#!/usr/bin/env bash
# Every refusal that stands for a decision has a file that triggers it
# and the message it must print, word for word. A rule whose message
# drifts is a rule that stopped teaching, and a rule that stops firing
# is a promise the dialect quietly dropped.
# `env bash`, not a login shell's own: nothing here is zsh's, and
# bash is the one interpreter both supported platforms have.
cd "$(dirname "$0")/.." || exit 1
pass=0; fail=0
for f in test/refuse/*.go; do
  want="${f%.go}.txt"
  got=$(./bin/gomamochi check "$f" 2>&1)
  if [ "$got" = "$(cat "$want" 2>/dev/null)" ]; then
    pass=$((pass+1))
  else
    fail=$((fail+1))
    echo "FAIL $(basename "$f")"
    diff "$want" <(echo "$got")
  fi
done
echo "REFUSALS: pass=$pass fail=$fail"
[ "$fail" -eq 0 ]
