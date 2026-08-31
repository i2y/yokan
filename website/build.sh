#!/usr/bin/env bash
# Build BOTH languages, always in this order: the English build
# cleans build/, the Japanese build then writes build/ja. Building
# only one of them silently loses the other — use this script.
set -e
cd "$(dirname "$0")"
.venv/bin/zensical build
.venv/bin/zensical build -f zensical.ja.toml
echo "site: build/ (en) + build/ja (ja)"
