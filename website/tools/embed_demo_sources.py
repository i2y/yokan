#!/usr/bin/env python3
"""Put each demo's source into the gallery page, under its shot.

The gallery is a page of usage samples, so the sample has to be on it.
Each `#### <name> — …` heading names a file under crates/yokan/demo/;
the source goes in a collapsed block after the screenshot, and running
this again refreshes what is already there rather than adding a second
copy. Both languages, since both pages carry the same headings.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
DEMOS = ROOT / "crates" / "yokan" / "demo"
PAGES = {
    ROOT / "website" / "docs" / "demos.md": "source",
    ROOT / "website" / "docs-ja" / "demos.md": "ソース",
}
MARK = "<!-- source -->"


def sources(name: str) -> list[tuple[str, str]]:
    """(label, text) for a demo: one file, or a directory's modules."""
    one = DEMOS / f"{name}.py"
    if one.is_file():
        return [(one.name, one.read_text())]
    many = DEMOS / name
    if many.is_dir():
        return [(f.name, f.read_text()) for f in sorted(many.glob("*.py"))]
    return []


def block(name: str, label: str) -> str:
    parts = []
    for fname, text in sources(name):
        body = "\n".join("    " + l if l else "" for l in text.rstrip().split("\n"))
        parts.append(f'??? note "{fname}"\n\n    ```python\n{body}\n    ```\n')
    return MARK + "\n" + "\n".join(parts) + MARK if parts else ""


def main() -> int:
    for page, label in PAGES.items():
        text = page.read_text()
        # Drop what a previous run put in, so this is idempotent — the
        # blank line before the block included, since the insertion
        # below adds one back. Without it every run left another.
        text = re.sub(r"\n\n" + re.escape(MARK) + r".*?" + re.escape(MARK) + r"\n", "\n",
                      text, flags=re.S)
        out, n = [], 0
        lines = text.split("\n")
        for i, line in enumerate(lines):
            out.append(line)
            m = re.match(r"^#### ([a-z_]+)", lines[i - 1] if i else "")
            if m and line.startswith("<img "):
                b = block(m.group(1), label)
                if b:
                    out.append("")
                    out.append(b)
                    n += 1
        page.write_text("\n".join(out))
        print(f"{page.relative_to(ROOT)}: {n} demos")
    return 0


if __name__ == "__main__":
    sys.exit(main())
