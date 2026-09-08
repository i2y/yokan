// The site's six tour pages, in both languages, cut out of TOUR.md and
// TOUR.ja.md.
//
// The repository's tour is the one text. The site shows it in six
// pages because a single page that long is not read, and cutting it
// here rather than keeping a second copy is what stops the two from
// drifting: a paragraph written once appears in both places or in
// neither. Links between sections are rewritten to point at the page
// the section landed on.
//
//	go run ./website/tools/tourpages            write docs/tour*.md and docs-ja/
//	go run ./website/tools/tourpages --check    fail if either is behind the tour
//
// Run from gomamochi/, the module root: the tour is read from there,
// and the pages are written under website/.
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"strings"
	"unicode"
)

// A page: the file, the heading each language gives it, the lede under
// that heading, and the sections it takes from the tour, in order.
type page struct {
	file string
	en   [2]string
	ja   [2]string
	take []string
}

var pages = []page{
	{
		file: "tour.md",
		en: [2]string{"The first app",
			"An app is a struct, its state is its fields, and a handler is a closure over them."},
		ja: [2]string{"はじめてのアプリ",
			"アプリは構造体で、状態はそのフィールドです。\nハンドラは、そのフィールドがそのまま見えるクロージャです。"},
		take: []string{"The smallest app", "Holding state"},
	},
	{
		file: "tour-logic.md",
		en: [2]string{"Views and control flow",
			"How a screen is built, broken into pieces, and driven by what the app holds."},
		ja: [2]string{"ビューと制御構造",
			"画面をどう組み立て、どう分け、アプリの持つもので動かすか。"},
		take: []string{"Writing views", "Control flow in a view", "Form controls", "Handlers",
			"Lists, charts, and rows built on demand", "Maps", "Value structs"},
	},
	{
		file: "tour-canvas.md",
		en: [2]string{"The canvas and the keyboard",
			"A grid of virtual pixels, and keys read as a device rather than waited for."},
		ja: [2]string{"キャンバスとキーボード",
			"仮想的な画素の格子と、知らせを待つのではなくこちらから尋ねるキーボード。"},
		take: []string{"The canvas", "The keyboard"},
	},
	{
		file: "tour-ui.md",
		en: [2]string{"Look, and the window",
			"The keywords every element takes, the palette, and what the window itself brings."},
		ja: [2]string{"見た目とウィンドウ",
			"すべての要素が取るキーワード、配色、そしてウィンドウ自身が持ってくるもの。"},
		take: []string{"The keywords every element takes", "Themes and animation", "The window"},
	},
	{
		file: "tour-lib.md",
		en: [2]string{"Go, data, and work",
			"Go's own library, the framework's, timers, work off the window's thread, and the window that follows your saves."},
		ja: [2]string{"Go とデータとタスク",
			"Go 自身のライブラリ、フレームワークのライブラリ、タイマー、ウィンドウの外でする処理、そして保存すると書き換わるウィンドウ。"},
		take: []string{"Go's own standard library", "The framework's standard library",
			"Timers and work off the window's thread", "While you are writing it"},
	},
	{
		file: "tour-ship.md",
		en: [2]string{"Verify and ship",
			"The gate, what Gomamochi refuses, the bundle, and what does not work yet."},
		ja: [2]string{"確かめて配る",
			"ゲート、Gomamochi が断る書き方、バンドル、そしてまだできないこと。"},
		take: []string{"Headless runs and the gate", "What Gomamochi refuses", "Shipping",
			"What does not work yet"},
	},
}

func fail(msg string) {
	fmt.Fprintln(os.Stderr, "tourpages:", msg)
	os.Exit(1)
}

// The module root is where this runs from: go.mod, and website/ beside it.
func root() string {
	wd, err := os.Getwd()
	if err != nil {
		fail(err.Error())
	}
	if _, err := os.Stat(filepath.Join(wd, "go.mod")); err != nil {
		fail("run from gomamochi/: go run ./website/tools/tourpages")
	}
	if st, err := os.Stat(filepath.Join(wd, "website")); err != nil || !st.IsDir() {
		fail("run from gomamochi/: go run ./website/tools/tourpages")
	}
	return wd
}

func slurp(path string) string {
	text, err := os.ReadFile(path)
	if err != nil {
		fail(err.Error())
	}
	return string(text)
}

// --- reading the tour ---------------------------------------------------------

var spaces = regexp.MustCompile(`\s+`)

// slug is GitHub's anchor for a heading, which is also the site's (both
// configs ask pymdownx for a unicode-preserving slug): lower-case, every
// rune that is not a letter, a digit, `_`, whitespace or `-` dropped,
// and each run of whitespace one `-`. A letter here is Unicode's, so
// every Japanese heading keeps its characters.
func slug(h string) string {
	var b strings.Builder
	for _, r := range strings.ToLower(h) {
		if unicode.IsLetter(r) || unicode.IsDigit(r) || r == '_' || r == '-' || unicode.IsSpace(r) {
			b.WriteRune(r)
		}
	}
	return spaces.ReplaceAllString(b.String(), "-")
}

var (
	introRe    = regexp.MustCompile(`(?ms)\A# [^\n]*\n(.*?)^## `)
	commentRe  = regexp.MustCompile(`(?s)<!--.*?-->`)
	blankRe    = regexp.MustCompile(`\n\s*\n`)
	anchorRe   = regexp.MustCompile(`\]\(#([^)]+)\)`)
	trailingRe = regexp.MustCompile(`\n{3,}\z`)
)

// identity is what the tour says before its first section: the
// paragraph that names the language. The site's first tour page carries
// it, the way the other languages' first pages do, so a reader who
// lands there from a search meets the language before the vocabulary.
// Only the first paragraph travels — what follows it in the file is
// about the file (the checker, the other language's copy) and the site
// says both elsewhere.
func identity(path string) string {
	m := introRe.FindStringSubmatch(slurp(path))
	if m == nil {
		fail(path + ": no intro before the first section")
	}
	intro := commentRe.ReplaceAllString(m[1], "") // the draft marker is the owner's, not a reader's
	for _, para := range blankRe.Split(intro, -1) {
		if strings.TrimSpace(para) != "" {
			return strings.TrimSpace(para)
		}
	}
	fail(path + ": the intro is empty")
	return ""
}

// sections is every `## ` heading of the tour, in order, with the text
// under each.
func sections(path string) ([]string, map[string]string) {
	var order []string
	body := map[string]string{}
	cur := ""
	for _, line := range strings.SplitAfter(slurp(path), "\n") {
		if strings.HasPrefix(line, "## ") {
			if head := strings.TrimRight(line[3:], " \t\r\n"); head != "" {
				cur = head
				order = append(order, cur)
				body[cur] = ""
				continue
			}
		}
		if cur != "" {
			body[cur] += line
		}
	}
	return order, body
}

// --- writing a page --------------------------------------------------------------

// What a language's pages are cut with: the tour's identity paragraph,
// the Japanese heading for each English one, and the page each anchor
// landed on.
type where struct {
	identity string
	jaOf     map[string]string
	page     map[string]string
}

func render(lang string, i int, body map[string]string, w *where) string {
	p := pages[i]
	title, lede := p.en[0], p.en[1]
	if lang == "ja" {
		title, lede = p.ja[0], p.ja[1]
	}
	var b strings.Builder
	b.WriteString("<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->\n")
	b.WriteString("# " + title + "\n\n")
	if i == 0 {
		b.WriteString(w.identity + "\n\n")
	}
	b.WriteString(lede + "\n")
	for _, head := range p.take {
		key := head
		if lang == "ja" {
			key = w.jaOf[head]
		}
		text, ok := body[key]
		if !ok {
			fail(fmt.Sprintf("%s: the tour has no section %q", p.file, head))
		}
		b.WriteString("\n## " + key + "\n" + text)
	}
	// A link to a section is rewritten to the page it landed on, and
	// dropped to a bare anchor when that is this page.
	out := anchorRe.ReplaceAllStringFunc(b.String(), func(m string) string {
		anchor := anchorRe.FindStringSubmatch(m)[1]
		file, ok := w.page[anchor]
		if !ok || file == p.file {
			return "](#" + anchor + ")"
		}
		return "](" + file + "#" + anchor + ")"
	})
	// The tour sits beside docs/PIXIE.md in the repository; the site does not.
	out = strings.ReplaceAll(out, "](../docs/PIXIE.md)", "](https://github.com/i2y/yokan/blob/main/docs/PIXIE.md)")
	return trailingRe.ReplaceAllString(out, "\n")
}

// --- main -------------------------------------------------------------------------

func main() {
	check := slices.Contains(os.Args[1:], "--check")
	dir := root()
	stale := 0

	enOrder, _ := sections(filepath.Join(dir, "TOUR.md"))
	for _, lang := range []string{"en", "ja"} {
		tour := filepath.Join(dir, "TOUR.md")
		docs := "docs"
		if lang == "ja" {
			tour = filepath.Join(dir, "TOUR.ja.md")
			docs = "docs-ja"
		}
		order, body := sections(tour)

		// The two tours carry the same sections in the same order, so
		// the Japanese heading for an English one is the heading in
		// that place.
		w := &where{jaOf: map[string]string{}, page: map[string]string{}}
		if lang == "ja" {
			if len(order) != len(enOrder) {
				fail("the two tours have a different number of sections")
			}
			for i, head := range enOrder {
				w.jaOf[head] = order[i]
			}
		}
		w.identity = identity(tour)
		for _, p := range pages {
			for _, head := range p.take {
				key := head
				if lang == "ja" {
					key = w.jaOf[head]
				}
				w.page[slug(key)] = p.file
			}
		}

		for i := range pages {
			text := render(lang, i, body, w)
			rel := filepath.Join("website", docs, pages[i].file)
			path := filepath.Join(dir, rel)
			if check {
				have, _ := os.ReadFile(path)
				if string(have) != text {
					fmt.Fprintf(os.Stderr, "FAIL %s/%s is behind the tour\n", lang, pages[i].file)
					stale++
				}
				continue
			}
			if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
				fail(err.Error())
			}
			fmt.Println("wrote", rel)
		}
	}
	if stale > 0 {
		os.Exit(1)
	}
}
