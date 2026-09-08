# Gomamochi documentation site

Source for the user-facing Gomamochi site, built with
[Zensical](https://zensical.org) (the Material for MkDocs successor by
the squidfunk team; latest release, unpinned).

## Layout

```
website/
├── zensical.toml     # English site (nav, palette, markdown extensions)
├── zensical.ja.toml  # Japanese site — docs-ja/ -> build/ja/
├── docs/             # English pages
├── docs-ja/          # Japanese pages (same pages, same images —
│                     #   real copies: zensical does not follow
│                     #   symlinked directories)
├── tools/            # the four generated pages, and the checker (Go)
└── README.md
```

Both configs carry `extra.alternate`, which renders the header language
switcher, and a unicode-preserving `toc.slugify` so Japanese heading
anchors match GitHub's.

## Build and serve

```console
$ cd gomamochi/website
$ uv venv .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh                             # both languages, right order
$ python3 -m http.server 8004 -d build   # preview both, incl. /ja/
```

The English build cleans `build/`, so a lone EN build silently drops
`build/ja` — always build through `./build.sh`. `just gomamochi-site`
and `just gomamochi-site-serve` from the repository root do both, and
the serve recipe puts the build under `/yokan/gomamochi/`, which is the
path the pages' absolute links are written for.

## The generated pages, and the checker

Four of the pages are written from somewhere else, so nothing on them
can drift from what the command actually does. Each writer is a Go
program, run from `gomamochi/` as `go run ./website/tools/<name>`, and
each takes `--check`.

- `tools/tourpages` cuts the six tour pages out of `TOUR.md` and
  `TOUR.ja.md`, rewriting a link to a section so it points at the page
  that section landed on. The repository's tour is the one text; this is
  the same text in six pieces.
- `tools/elementspage` writes `elements.md` in both languages from
  `crates/pixie-capi/elements.toml`, in Go spelling: every element's
  constructor, every keyword as the method it is, its type and its
  default, and the canvas's drawing commands as `Painter` methods. It
  reads `elements.go` back to confirm every name it prints is there.
- `tools/demospage` writes `demos.md` from `demo/`: each app's
  screenshot and, under it, its whole source. The gloss for each one is
  in that program, and a demo with no entry stops it.
- `tools/refusalspage` writes `refusals.md` from `test/refuse/`,
  quoting each refusal's message from the file that holds its wording.
- `tools/sitecheck` runs all four with `--check` and then reads what is
  left: that both languages carry the same pages, that the nav and the
  tree agree, that every image a page points at is there, and that the
  gallery has a picture of every demo. The sweep runs the checker, not
  the writers.

The gallery's pictures are copies of `demo/screenshots/`. Refresh them
into `docs/images/demos/` and `docs-ja/images/demos/` when a demo's
first screen changes.

## Where it deploys

`site_url` is `https://i2y.github.io/yokan/gomamochi/`. One repository
means one Pages site, so Yokan's is the root and this one sits under
it; `.github/workflows/docs.yml` builds all four on a push to `main`
and deploys them as one artifact. The site is still written to stand on
its own: moving it to `i2y/gomamochi` means changing `site_url` and the
two `extra.alternate` links, and nothing else.
