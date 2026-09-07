#!/bin/sh
# Fetch and build the perl Rakugan's apps are measured under, into
# ~/.cache/perl/<version>, and print the directory.
#
#   rakugan/tools/perl_setup.sh          # build if absent, print the dir
#   rakugan/tools/perl_setup.sh --path   # print the dir, build nothing
#
# The pin lives here and nowhere else: bin/rakugan asks this file. Any
# perl of 5.40 or newer runs an app (RAKUGAN_PERL=/path/to/perl points
# at one); this is the one the sweep is run with. Bumping it is its own
# commit, with the sweep green.
# How many jobs to build with. `sysctl -n hw.ncpu` is macOS's answer and
# `nproc` is Linux's; either missing, one job still builds.
build_jobs() {
  nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 1
}

set -eu

PERL_PIN=5.44.0
# perl-5.44.0.tar.gz as fetched from www.cpan.org on 2026-09-06.
PERL_SHA256=3b855066b92491cb40e86affb1ca57d1a388aa43e51b91c7806a32c2f65f96c3

DIR="$HOME/.cache/perl/$PERL_PIN"
SRC="$HOME/.cache/perl/src"

if [ "${1:-}" = "--path" ]; then
  echo "$DIR"
  exit 0
fi

if [ ! -x "$DIR/bin/perl" ]; then
  mkdir -p "$SRC"
  tarball="$SRC/perl-$PERL_PIN.tar.gz"
  if [ ! -f "$tarball" ]; then
    echo "rakugan: fetching perl $PERL_PIN" >&2
    curl -fsSL -o "$tarball" "https://www.cpan.org/src/5.0/perl-$PERL_PIN.tar.gz"
  fi
  echo "$PERL_SHA256  $tarball" | shasum -a 256 -c - >/dev/null
  echo "rakugan: building perl $PERL_PIN into $DIR (a few minutes, once)" >&2
  rm -rf "$SRC/perl-$PERL_PIN"
  tar xzf "$tarball" -C "$SRC"
  (
    cd "$SRC/perl-$PERL_PIN" &&
    sh Configure -des -Dprefix="$DIR" -Dusethreads > "$SRC/configure.log" 2>&1 &&
    make -j"$(build_jobs)" > "$SRC/make.log" 2>&1 &&
    make install > "$SRC/install.log" 2>&1
  ) || { echo "rakugan: the perl build failed; see $SRC/*.log" >&2; exit 1; }
fi

echo "$DIR"
