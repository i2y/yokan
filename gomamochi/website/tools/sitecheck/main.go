// What the site claims, checked against the tree it claims it about.
//
// Four of the site's pages are written from somewhere else: the tour
// from `TOUR.md`, the elements from the table, the gallery from
// `demo/`, the refusals from the fixtures that hold their wording. Each
// writer has a `--check`, and this program runs all of them, then reads
// what is left: that both languages carry the same pages, that the nav
// and the tree agree, that every image a page points at is there, and
// that the gallery has a picture of every demo. It reads only; nothing
// here rewrites a page.
//
//	go run ./website/tools/sitecheck
//
// Run from gomamochi/, the module root.
package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
)

var fails []string

func ok(msg string) { fmt.Println("OK   " + msg) }

func die(msg string) {
	fmt.Fprintln(os.Stderr, "sitecheck:", msg)
	os.Exit(1)
}

// The module root is where this runs from: go.mod, and website/ beside it.
func root() string {
	wd, err := os.Getwd()
	if err != nil {
		die(err.Error())
	}
	if _, err := os.Stat(filepath.Join(wd, "go.mod")); err != nil {
		die("run from gomamochi/: go run ./website/tools/sitecheck")
	}
	if st, err := os.Stat(filepath.Join(wd, "website")); err != nil || !st.IsDir() {
		die("run from gomamochi/: go run ./website/tools/sitecheck")
	}
	return wd
}

func slurp(path string) string {
	text, err := os.ReadFile(path)
	if err != nil {
		die(err.Error())
	}
	return string(text)
}

// pagesIn is every .md under a docs dir, sorted.
func pagesIn(site, dir string) []string {
	entries, err := os.ReadDir(filepath.Join(site, dir))
	if err != nil {
		die(err.Error())
	}
	var md []string
	for _, e := range entries {
		if n := e.Name(); strings.HasSuffix(n, ".md") && !e.IsDir() {
			md = append(md, n)
		}
	}
	sort.Strings(md)
	return md
}

var (
	navRe   = regexp.MustCompile(`(?ms)^nav = \[(.*?)^\]`)
	pageRe  = regexp.MustCompile(`"([\w-]+\.md)"`)
	imageRe = regexp.MustCompile(`(?:!\[[^\]]*\]\(|<img src=")(images/[^)"#]+)`)
)

func main() {
	dir := root()
	site := filepath.Join(dir, "website")

	// --- the four generated pages ---------------------------------------------
	for _, gen := range []string{"tourpages", "elementspage", "demospage", "refusalspage"} {
		cmd := exec.Command("go", "run", "./website/tools/"+gen, "--check")
		cmd.Dir = dir
		cmd.Stdout, cmd.Stderr = os.Stdout, os.Stderr
		if err := cmd.Run(); err == nil {
			ok(gen)
		} else {
			fails = append(fails, gen+" is behind its source (run go run ./website/tools/"+gen+")")
		}
	}

	// --- both languages carry the same pages ----------------------------------
	en, ja := pagesIn(site, "docs"), pagesIn(site, "docs-ja")
	if strings.Join(en, " ") == strings.Join(ja, " ") {
		ok(fmt.Sprintf("both languages: %d pages", len(en)))
	} else {
		has := func(list []string) map[string]bool {
			m := map[string]bool{}
			for _, p := range list {
				m[p] = true
			}
			return m
		}
		inEn, inJa := has(en), has(ja)
		var onlyEn, onlyJa []string
		for _, p := range en {
			if !inJa[p] {
				onlyEn = append(onlyEn, p)
			}
		}
		for _, p := range ja {
			if !inEn[p] {
				onlyJa = append(onlyJa, p)
			}
		}
		fails = append(fails, "the two languages differ: only EN "+strings.Join(onlyEn, ", ")+
			"; only JA "+strings.Join(onlyJa, ", "))
	}

	// --- every page in the nav exists, and every page is in the nav -----------
	for _, pair := range [][2]string{{"zensical.toml", "docs"}, {"zensical.ja.toml", "docs-ja"}} {
		config, docs := pair[0], pair[1]
		text := slurp(filepath.Join(site, config))
		m := navRe.FindStringSubmatch(text)
		if m == nil {
			fails = append(fails, config+" has no nav")
			continue
		}
		var inNav []string
		for _, p := range pageRe.FindAllStringSubmatch(m[1], -1) {
			inNav = append(inNav, p[1])
		}
		files := pagesIn(site, docs)
		isFile, isNav := map[string]bool{}, map[string]bool{}
		for _, f := range files {
			isFile[f] = true
		}
		for _, p := range inNav {
			isNav[p] = true
		}
		var gone, unlisted []string
		for _, p := range inNav {
			if !isFile[p] {
				gone = append(gone, p)
			}
		}
		for _, f := range files {
			if !isNav[f] {
				unlisted = append(unlisted, f)
			}
		}
		if len(gone) > 0 {
			fails = append(fails, config+" lists a page that is not there: "+strings.Join(gone, " "))
		}
		if len(unlisted) > 0 {
			fails = append(fails, docs+" has a page the nav never shows: "+strings.Join(unlisted, " "))
		}
		if len(gone) == 0 && len(unlisted) == 0 {
			ok(fmt.Sprintf("%s: %d pages in the nav", config, len(inNav)))
		}
	}

	// --- images: every page's images exist ------------------------------------
	for _, docs := range []string{"docs", "docs-ja"} {
		var missing []string
		for _, page := range pagesIn(site, docs) {
			text := slurp(filepath.Join(site, docs, page))
			seen := map[string]bool{}
			for _, m := range imageRe.FindAllStringSubmatch(text, -1) {
				img := m[1]
				if seen[img] {
					continue
				}
				seen[img] = true
				if _, err := os.Stat(filepath.Join(site, docs, img)); err != nil {
					missing = append(missing, page+" → "+img)
				}
			}
		}
		if len(missing) > 0 {
			fails = append(fails, docs+" points at images that are not there: "+strings.Join(missing, " "))
		} else {
			ok(docs + ": every image is there")
		}
	}

	// --- the gallery has a picture of every demo ------------------------------
	entries, err := os.ReadDir(filepath.Join(dir, "demo"))
	if err != nil {
		die(err.Error())
	}
	var apps, unshot []string
	for _, e := range entries {
		if n := e.Name(); strings.HasSuffix(n, ".go") && !e.IsDir() {
			apps = append(apps, strings.TrimSuffix(n, ".go"))
		}
	}
	sort.Strings(apps)
	for _, n := range apps {
		pictured := false
		for _, ext := range []string{"png", "gif"} {
			if _, err := os.Stat(filepath.Join(site, "docs", "images", "demos", n+"."+ext)); err == nil {
				pictured = true
			}
		}
		if !pictured {
			unshot = append(unshot, n)
		}
	}
	if len(unshot) > 0 {
		fails = append(fails, "the gallery has no picture of: "+strings.Join(unshot, " "))
	} else {
		ok(fmt.Sprintf("the gallery: %d demos, all pictured", len(apps)))
	}

	if len(fails) > 0 {
		for _, f := range fails {
			fmt.Fprintln(os.Stderr, "FAIL "+f)
		}
		os.Exit(1)
	}
	fmt.Println("SITE OK")
}
