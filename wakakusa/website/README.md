# Wakakusa documentation site

Source for the user-facing Wakakusa site, built with
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
├── tools/            # the two generated pages, and the checker
└── README.md
```

Both configs carry `extra.alternate`, which renders the header
language switcher, and a unicode-preserving `toc.slugify` so
Japanese heading anchors match GitHub's.

The tour pages are ports of `TOUR.md` / `TOUR.ja.md` with site-local
links; when the tour changes, re-port them (the substance must not
drift). Every complete example on them is one `tools/tour_check.rb`
already runs, so an example here cannot be one the compiler refuses.

## Build and serve

```console
$ cd wakakusa/website
$ uv venv .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh                             # both languages, right order
$ python3 -m http.server 8002 -d build   # preview both, incl. /ja/
```

The English build cleans `build/`, so a lone EN build silently drops
`build/ja` — always build through `./build.sh`. `just wakakusa-site`
and `just wakakusa-site-serve` from the repository root do both, and
the serve recipe puts the build under `/wakakusa/`, which is the path
the pages' absolute links are written for.

## The generated pages, and the checker

- `tools/elements_page.rb` writes `elements.md` in both languages from
  `elements.toml` — every element, every keyword, its type and its
  default. Nothing on that page is typed by hand, so it cannot drift
  from what an app may actually write.
- `tools/embed_demo_sources.rb` puts each demo's source into the
  gallery under its screenshot, reading `wakakusa/demo/`. Run it
  whenever a demo changes; it replaces what a previous run left rather
  than adding a second copy.
- `tools/site_check.rb` fails when the refusals page quotes a message
  the fixtures under `test/refuse/` no longer print, and when a demo
  has no screenshot on the gallery. Run it after either of the above.

## Where it deploys

`site_url` is `https://i2y.github.io/yokan/wakakusa/`. One repository
means one Pages site, so Yokan's is the root and this one sits under it;
`.github/workflows/docs.yml` builds all three on a push to `main` and
deploys them as one artifact. The site is still written to stand on its
own: moving it to `i2y/wakakusa` means changing `site_url` and the two
`extra.alternate` links, and nothing else.
