// Go's own standard library, under the gate.
//
// Nothing here is Gomamochi's. `math`, `sort`, `time`, `encoding/json`,
// `encoding/csv`, `strings` and `regexp` are the language's, and both
// runs call the same compiled packages. What the gate says is that
// they answer the same — which is the only claim worth making about a
// standard library shared between an interpreter and a compiler.
package main

import (
	"encoding/csv"
	"encoding/json"
	"fmt"
	"math"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	. "github.com/i2y/yokan/gomamochi"
)

type Stdlib struct {
	hyp    float64
	spread string
	sift   string
	tally  string
	runs   string
	stamp  string
	doc    string
	row    string
	words  string
	unique string
	scores []int
	votes  []string
}

func (s *Stdlib) measure() {
	s.hyp = math.Sqrt(3*3 + 4*4)
}

func (s *Stdlib) stats() {
	sum := 0
	for _, v := range s.scores {
		sum += v
	}
	mean := float64(sum) / float64(len(s.scores))
	sorted := append([]int{}, s.scores...)
	sort.Ints(sorted)
	median := sorted[len(sorted)/2]
	s.spread = fmt.Sprintf("mean %.1f median %d min %d max %d", mean, median, sorted[0], sorted[len(sorted)-1])
}

func (s *Stdlib) doSift() {
	var big, small []string
	for _, v := range s.scores {
		if v > 5 {
			big = append(big, strconv.Itoa(v))
		} else {
			small = append(small, strconv.Itoa(v))
		}
	}
	s.sift = fmt.Sprintf("big %s small %s", strings.Join(big, ","), strings.Join(small, ","))
}

// A count per name, most votes first and then by name. The map is
// walked in a handler, and what the view reads is the sorted list.
func (s *Stdlib) count() {
	counts := map[string]int{}
	for _, v := range s.votes {
		counts[v]++
	}
	type pair struct {
		name string
		n    int
	}
	var pairs []pair
	for name, n := range counts {
		pairs = append(pairs, pair{name, n})
	}
	sort.Slice(pairs, func(i, j int) bool {
		if pairs[i].n != pairs[j].n {
			return pairs[i].n > pairs[j].n
		}
		return pairs[i].name < pairs[j].name
	})
	var parts []string
	for _, p := range pairs {
		parts = append(parts, fmt.Sprintf("%s:%d", p.name, p.n))
	}
	s.tally = strings.Join(parts, " ")
}

func (s *Stdlib) combine() {
	var steps, pairs []string
	for i := 1; i < len(s.scores); i++ {
		steps = append(steps, strconv.Itoa(s.scores[i]-s.scores[i-1]))
	}
	for i, letter := range []string{"a", "b", "c"} {
		pairs = append(pairs, fmt.Sprintf("%s%d", letter, s.scores[i]))
	}
	s.runs = fmt.Sprintf("steps %s pairs %s", strings.Join(steps, ","), strings.Join(pairs, ","))
}

func (s *Stdlib) doStamp() {
	s.stamp = time.Unix(1700000000, 0).UTC().Format("2006-01-02 15:04:05 UTC")
}

func (s *Stdlib) parse() {
	src := `{"name": "gomamochi", "parts": [1, 2, 3], "ok": true}`
	var doc map[string]any
	if err := json.Unmarshal([]byte(src), &doc); err != nil {
		s.doc = "unreadable"
		return
	}
	parts, _ := doc["parts"].([]any)
	sum := 0.0
	for _, p := range parts {
		v, _ := p.(float64)
		sum += v
	}
	s.doc = fmt.Sprintf("%v %d %v", doc["name"], int(sum), doc["ok"])
}

func (s *Stdlib) write() {
	top := s.scores[0]
	for _, v := range s.scores {
		if v > top {
			top = v
		}
	}
	out, _ := json.Marshal(map[string]any{"n": len(s.scores), "top": top})
	s.doc = string(out)
}

func (s *Stdlib) readRow() {
	fields, err := csv.NewReader(strings.NewReader("api,42,\"one, two\"")).Read()
	if err != nil {
		s.row = "unreadable"
		return
	}
	s.row = strings.Join(fields, " | ")
}

func (s *Stdlib) doWords() {
	line := "  the quick brown fox  "
	var caps []string
	for _, w := range strings.Fields(line) {
		caps = append(caps, strings.ToUpper(w[:1])+w[1:])
	}
	s.words = fmt.Sprintf("%s (%d)", strings.Join(caps, "-"), len(strings.TrimSpace(line)))
}

// The set of names, without repeats.
func (s *Stdlib) doUnique() {
	seen := map[string]bool{}
	var names []string
	for _, v := range s.votes {
		if !seen[v] {
			seen[v] = true
			names = append(names, v)
		}
	}
	sort.Strings(names)
	s.unique = strings.Join(names, ",")
}

func (s *Stdlib) findNumbers() {
	sum := 0
	for _, m := range regexp.MustCompile(`\d+`).FindAllString("a1b22c333", -1) {
		v, _ := strconv.Atoi(m)
		sum += v
	}
	s.spread = strconv.Itoa(sum)
}

func (s *Stdlib) View() Element {
	return Column(
		Text("Go's own, in both runs").Size(16).Bold(true),
		Text(fmt.Sprintf("hypotenuse: %v", s.hyp)),
		Text("spread: "+s.spread),
		Text("sift: "+s.sift),
		Text("tally: "+s.tally),
		Text("runs: "+s.runs),
		Text("stamp: "+s.stamp),
		Text("json: "+s.doc),
		Text("csv: "+s.row),
		Text("words: "+s.words),
		Text("set: "+s.unique),
		Row(
			Button("measure").OnClick(func() { s.measure() }),
			Button("stats").OnClick(func() { s.stats() }),
			Button("sift").OnClick(func() { s.doSift() }),
			Button("count").OnClick(func() { s.count() }),
		).Spacing(6),
		Row(
			Button("combine").OnClick(func() { s.combine() }),
			Button("stamp").OnClick(func() { s.doStamp() }),
			Button("parse").OnClick(func() { s.parse() }),
			Button("write").OnClick(func() { s.write() }),
		).Spacing(6),
		Row(
			Button("csv").OnClick(func() { s.readRow() }),
			Button("words").OnClick(func() { s.doWords() }),
			Button("set").OnClick(func() { s.doUnique() }),
			Button("scan").OnClick(func() { s.findNumbers() }),
		).Spacing(6),
	).Spacing(6).Padding(14)
}

func main() {
	Run(&Stdlib{
		spread: "-", sift: "-", tally: "-", runs: "-", stamp: "-", doc: "-", row: "-", words: "-", unique: "-",
		scores: []int{3, 5, 8, 13, 21},
		votes:  []string{"ivy", "momo", "ivy", "ada", "momo", "ivy", "ada"},
	}, Title("stdlib"))
}
