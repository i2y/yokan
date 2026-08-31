# Yokan documentation site

Source for the user-facing Yokan site, built with
[Zensical](https://zensical.org) (the Material for MkDocs successor
by the squidfunk team; latest release, unpinned).

## Layout

```
website/
├── zensical.toml     # English site (nav, palette, markdown extensions)
├── zensical.ja.toml  # Japanese site — docs-ja/ -> build/ja/
├── docs/             # English pages
├── docs-ja/          # Japanese pages (same pages, same images —
│                     #   real copies: zensical does not follow
│                     #   symlinked directories)
└── README.md
```

Both configs carry `extra.alternate`, which renders the header
language switcher, and a unicode-preserving `toc.slugify` so
Japanese heading anchors match GitHub's. `tour.md` and `demos.md`
(both languages) are ports of the in-repo documents with site-local
links; when those documents change, re-port them (the substance
must not drift).

## Build and serve

```console
$ cd website
$ uv venv .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh                             # both languages, right order
$ python3 -m http.server 8001 -d build   # preview both, incl. /ja/
$ .venv/bin/zensical serve               # EN-only live preview on :8000
```

The English build cleans `build/`, so a lone EN build silently
drops `build/ja` — always build through `./build.sh`.

Deployed by `.github/workflows/docs.yml` to GitHub Pages on every
push to main that touches `website/`.
