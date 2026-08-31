#!/usr/bin/env python3
"""Concretize a vendored Zed crate manifest (DESIGN section 5, 8.11).

Resolves `workspace = true` inheritance against the Zed workspace
root: internal path deps become git+rev deps at the pinned revision,
external versions inline verbatim, `[lints]` drops. Usage:

    cp -R ~/.cargo/git/checkouts/zed-*/<rev>/crates/<crate> vendor/<crate>
    tools/devendor.py ~/.cargo/git/checkouts/zed-*/<rev>/Cargo.toml \
        vendor/<crate>/Cargo.toml <rev>

Then re-apply the carried patches (P1 in vendor/gpui_macos), add the
crate to the `[patch."https://github.com/zed-industries/zed"]` tables
(workspace root; generated crates pick it up via the CLI), and run the
tier gate. Keep the vendor set minimal: one crate per needed patch —
platforms whose crates need no patch ride the git pin untouched.
"""
import re
import sys
import tomllib

ZED_ROOT_TOML = sys.argv[1]
TARGET = sys.argv[2]
GIT = "https://github.com/zed-industries/zed"
REV = sys.argv[3] if len(sys.argv) > 3 else "d9ad6aff67e47de43abb270d22de75dd950f1b48"

with open(ZED_ROOT_TOML, "rb") as f:
    root = tomllib.load(f)
ws_deps = root["workspace"]["dependencies"]
ws_pkg = root["workspace"]["package"]


def toml_value(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return '"%s"' % v
    if isinstance(v, list):
        return "[%s]" % ", ".join(toml_value(x) for x in v)
    raise SystemExit(f"unhandled value {v!r}")


def concrete_dep(name):
    if name not in ws_deps:
        raise SystemExit(f"{name}: not in workspace.dependencies")
    entry = ws_deps[name]
    if isinstance(entry, str):
        return f'{name} = "{entry}"'
    entry = dict(entry)
    parts = []
    if "path" in entry:
        entry.pop("path")
        parts.append(f'git = "{GIT}"')
        parts.append(f'rev = "{REV}"')
    for k, v in entry.items():
        parts.append(f"{k} = {toml_value(v)}")
    return f"{name} = {{ {', '.join(parts)} }}"


out = []
lines = open(TARGET).read().splitlines()
i = 0
while i < len(lines):
    line = lines[i]
    if line.strip() == "[lints]":
        # Drop the [lints] section (workspace-inherited).
        i += 1
        while i < len(lines) and not lines[i].startswith("["):
            i += 1
        continue
    m = re.match(r"^edition\.workspace = true$", line)
    if m:
        out.append(f'edition = "{ws_pkg["edition"]}"')
        i += 1
        continue
    if re.match(r"^publish\.workspace = true$", line):
        i += 1
        continue
    m = re.match(r"^([A-Za-z0-9_-]+)\.workspace = true$", line)
    if m:
        out.append(concrete_dep(m.group(1)))
        i += 1
        continue
    m = re.match(r"^([A-Za-z0-9_-]+) = \{ workspace = true(?:, )?(.*)\}$", line)
    if m:
        name, rest = m.group(1), m.group(2).strip().rstrip(",").strip()
        base = concrete_dep(name)
        if rest:
            assert base.endswith(" }"), base
            base = base[:-2] + ", " + rest + " }"
        out.append(base)
        i += 1
        continue
    out.append(line)
    i += 1

open(TARGET, "w").write("\n".join(out) + "\n")
print("rewrote", TARGET)
