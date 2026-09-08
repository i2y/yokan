// Every complete app in the tour, run.
//
// A fenced `go` block that is a `package main` and ends in a call to
// `Run` is an app, not an illustration, so it is written out and put
// through the same command a demo goes through. A `<!-- script: … -->`
// line just above the fence says what to drive it with; without one
// the block is only checked, which is what an example with no state to
// change needs.
//
// They are written into `demo/.gate/tour/`, beside every other
// generated thing. The point is that the tour cannot drift: a rename in
// the vocabulary or a new refusal breaks the page that teaches it,
// here, before a reader meets it.
//
//	go run ./tools/tourcheck TOUR.md TOUR.ja.md website/docs/tour*.md
package main

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

type app struct {
	script string
	body   string
}

var scriptRe = regexp.MustCompile(`<!--\s*script:\s*(.*?)\s*-->`)

func apps(path string) []app {
	f, err := os.Open(path)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	defer f.Close()
	var found []app
	var fence []string
	inFence := false
	script := ""
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 1<<20), 1<<20)
	for sc.Scan() {
		line := sc.Text()
		if inFence {
			if strings.HasPrefix(line, "```") {
				body := strings.Join(fence, "\n") + "\n"
				if strings.Contains(body, "package main") && strings.Contains(body, "Run(") {
					found = append(found, app{script, body})
				}
				inFence, fence, script = false, nil, ""
			} else {
				fence = append(fence, line)
			}
			continue
		}
		if m := scriptRe.FindStringSubmatch(line); m != nil {
			script = m[1]
		} else if strings.HasPrefix(line, "```go") {
			inFence = true
		} else if strings.TrimSpace(line) != "" {
			script = ""
		}
	}
	return found
}

func main() {
	root, err := filepath.Abs(filepath.Join(filepath.Dir(os.Args[0]), "..", ".."))
	if wd, e := os.Getwd(); e == nil {
		root = wd
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	out := filepath.Join(root, "demo", ".gate", "tour")
	os.MkdirAll(out, 0o755)
	pages := os.Args[1:]
	if len(pages) == 0 {
		pages = []string{"TOUR.md"}
	}
	failed := 0
	for _, page := range pages {
		list := apps(page)
		if len(list) == 0 {
			fmt.Fprintf(os.Stderr, "%s: no complete app in it\n", filepath.Base(page))
		}
		// The whole path names the file, so a page under website/ cannot
		// overwrite the tour page of the same name.
		stem := strings.ToLower(strings.TrimSuffix(strings.TrimPrefix(page, "./"), ".md"))
		stem = strings.NewReplacer("/", "_", ".", "_", "-", "_").Replace(stem)
		for i, a := range list {
			file := filepath.Join(out, fmt.Sprintf("%s_%02d.go", stem, i))
			if err := os.WriteFile(file, []byte(a.body), 0o644); err != nil {
				fmt.Fprintln(os.Stderr, err)
				os.Exit(1)
			}
			args := []string{"check", file}
			if a.script != "" {
				args = []string{"gate", file, "--script", a.script}
			}
			cmd := exec.Command(filepath.Join(root, "bin", "gomamochi"), args...)
			cmd.Dir = root
			ok := cmd.Run() == nil
			mark := "OK"
			if !ok {
				mark = "FAIL"
				failed++
			}
			note := ""
			if a.script != "" {
				note = "  (" + a.script + ")"
			}
			fmt.Printf("%-4s %s%s\n", mark, filepath.Base(file), note)
		}
	}
	if failed > 0 {
		fmt.Printf("TOUR: %d failed\n", failed)
		os.Exit(1)
	}
	fmt.Println("TOUR: every example runs")
}
