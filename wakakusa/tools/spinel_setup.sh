#!/bin/sh
# Fetch and build the spinel Wakakusa is pinned to, into
# ~/.cache/spinel/<sha>, and print the directory.
#
#   wakakusa/tools/spinel_setup.sh          # build if absent, print the dir
#   wakakusa/tools/spinel_setup.sh --path   # print the dir, build nothing
#
# The pin lives here and nowhere else: bin/wakakusa reads SPINEL_PIN out
# of this file. Bumping it is its own commit, with the sweep green.
#
# Never point Wakakusa at another checkout of spinel unless it is 2026-09
# or newer: ffi_callback, ffi_source and --int-overflow are what the door
# is written against.
set -eu

SPINEL_PIN=b64b120
SPINEL_REPO=https://github.com/matz/spinel

# How many jobs to build with. `sysctl -n hw.ncpu` is macOS's answer and
# `nproc` is Linux's; either missing, one job still builds.
build_jobs() {
  nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 1
}

DIR="$HOME/.cache/spinel/$SPINEL_PIN"

if [ "${1:-}" = "--path" ]; then
  echo "$DIR"
  exit 0
fi

if [ ! -x "$DIR/spinel" ]; then
  if [ ! -d "$DIR/.git" ]; then
    echo "wakakusa: fetching spinel $SPINEL_PIN into $DIR" >&2
    rm -rf "$DIR"
    mkdir -p "$(dirname "$DIR")"
    git clone --quiet --no-checkout --filter=blob:none "$SPINEL_REPO" "$DIR"
    git -C "$DIR" checkout --quiet "$SPINEL_PIN"
  fi
  echo "wakakusa: building spinel $SPINEL_PIN (a few minutes, once)" >&2
  (cd "$DIR" && make deps >/dev/null && make -j"$(build_jobs)" >/dev/null)
fi

echo "$DIR"
