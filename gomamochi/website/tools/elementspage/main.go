// The site's Elements page, in both languages, written from the table.
//
// `crates/pixie-capi/elements.toml` is the one table: what the engine
// draws, what `tools/gen` writes the Go an app calls from (elements.go)
// and the interpreter's view of the package from, and what the other
// three languages on this engine read. A reference page written from
// anything else would be a second opinion, so this one is written from
// the table, in the Go spelling `tools/gen` gives it — and every
// constructor and method it is about to list is looked up in elements.go
// first, so a spelling the code does not have stops the program rather
// than reaching a reader. Nothing here is typed by hand except the prose
// that frames it.
//
//	go run ./website/tools/elementspage            write docs/elements.md and docs-ja/
//	go run ./website/tools/elementspage --check    fail if either is behind the table
//
// Run from gomamochi/, the module root. The TOML reader below is the one
// tools/gen carries — the subset the table is written in — copied rather
// than imported, since that program is a `main` too.
package main

import (
	"bytes"
	"fmt"
	"go/ast"
	"go/parser"
	"go/printer"
	"go/token"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"strconv"
	"strings"
)

func fail(msg string) {
	fmt.Fprintln(os.Stderr, "elementspage:", msg)
	os.Exit(1)
}

// The module root is where this runs from: go.mod, and website/ beside it.
func root() string {
	wd, err := os.Getwd()
	if err != nil {
		fail(err.Error())
	}
	if _, err := os.Stat(filepath.Join(wd, "go.mod")); err != nil {
		fail("run from gomamochi/: go run ./website/tools/elementspage")
	}
	if st, err := os.Stat(filepath.Join(wd, "website")); err != nil || !st.IsDir() {
		fail("run from gomamochi/: go run ./website/tools/elementspage")
	}
	return wd
}

// --- the TOML this file speaks (the subset tools/gen reads) -------------------------

type table map[string]any

func tomlValue(s string) any {
	switch {
	case strings.HasPrefix(s, `"`):
		v, err := strconv.Unquote(s)
		if err != nil {
			panic(err)
		}
		return v
	case s == "true":
		return true
	case s == "false":
		return false
	case s == "[]":
		return []any{}
	}
	if i, err := strconv.ParseInt(s, 10, 64); err == nil {
		return i
	}
	f, err := strconv.ParseFloat(s, 64)
	if err != nil {
		panic("not a TOML value: " + s)
	}
	return f
}

var inlineRe = regexp.MustCompile(`(\w+)\s*=\s*("(?:[^"\\]|\\.)*"|\[\]|[^,}\s]+)`)

func tomlInline(s string) table {
	t := table{}
	for _, m := range inlineRe.FindAllStringSubmatch(s, -1) {
		t[m[1]] = tomlValue(m[2])
	}
	return t
}

var headerRe = regexp.MustCompile(`^\[\[(\w+)\]\]$`)

func parseTOML(text string) map[string][]table {
	doc := map[string][]table{}
	var cur table
	var key string
	var arr []any
	inArr := false
	for _, raw := range strings.Split(text, "\n") {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if inArr {
			if strings.HasPrefix(line, "]") {
				cur[key] = arr
				inArr = false
			} else if strings.HasPrefix(line, "{") {
				arr = append(arr, tomlInline(line))
			}
			continue
		}
		if m := headerRe.FindStringSubmatch(line); m != nil {
			cur = table{}
			doc[m[1]] = append(doc[m[1]], cur)
			continue
		}
		k, v, _ := strings.Cut(line, "=")
		k, v = strings.TrimSpace(k), strings.TrimSpace(v)
		if v == "[" {
			key, arr, inArr = k, []any{}, true
			continue
		}
		cur[k] = tomlValue(v)
	}
	return doc
}

func (t table) str(k string) string {
	if v, ok := t[k].(string); ok {
		return v
	}
	return ""
}

func (t table) is(k string) bool { v, _ := t[k].(bool); return v }

func (t table) has(k string) bool { _, ok := t[k]; return ok }

func propNamed(el table, name string) table {
	for _, p := range el.props() {
		if p.str("name") == name {
			return p
		}
	}
	return nil
}

func (t table) props() []table {
	var out []table
	if vs, ok := t["props"].([]any); ok {
		for _, v := range vs {
			out = append(out, v.(table))
		}
	}
	return out
}

// --- the table ---------------------------------------------------------------------

var (
	riders   []table
	elements []table
	ops      []table
	element  = map[string]table{}
)

func load(path string) {
	text, err := os.ReadFile(path)
	if err != nil {
		fail(err.Error())
	}
	doc := parseTOML(string(text))
	riders, elements, ops = doc["rider"], doc["element"], doc["op"]
	for _, e := range elements {
		element[e.str("name")] = e
	}
}

// The order a reader meets them, which is not the order the table is
// written in: the table is ordered by the numbers the two sides count
// with, and those may never be reordered.
var groups = []struct {
	key   string
	names []string
}{
	{"text", []string{"text", "button", "link"}},
	{"arrange", []string{"column", "row", "grid", "grid_cell", "stack", "scroll_view", "h_scroll_view", "modal"}},
	{"fields", []string{"text_field", "number_field", "int_field", "checkbox", "switch", "slider",
		"select", "radio_group", "segmented", "tab_bar"}},
	{"rows", []string{"list_view", "table", "data_table"}},
	{"charts", []string{"bar_chart", "line_chart", "progress"}},
	{"pictures", []string{"image", "svg", "canvas"}},
	{"small", []string{"spacer", "divider", "spinner"}},
}

// --- spelling, as tools/gen spells it ------------------------------------------------

// camel is the Go spelling of a table name: `text_field` is TextField,
// `a11y_label` is A11yLabel, `h_scroll_view` is HScrollView.
func camel(name string) string {
	var b strings.Builder
	for _, part := range strings.Split(name, "_") {
		if part == "" {
			continue
		}
		b.WriteString(strings.ToUpper(part[:1]) + part[1:])
	}
	return b.String()
}

// lower is the same with a small first letter, for an argument.
func lower(name string) string {
	c := camel(name)
	return strings.ToLower(c[:1]) + c[1:]
}

// goType is what a prop's value is in Go.
func goType(p table) string {
	if p.has("handler") {
		switch p.str("handler") {
		case "none":
			return "func()"
		case "text":
			return "func(string)"
		case "bool":
			return "func(bool)"
		case "int":
			return "func(int)"
		case "float":
			return "func(float64)"
		}
	}
	switch p.str("type") {
	case "str":
		return "string"
	case "num":
		return "float64"
	case "int":
		return "int"
	case "bool":
		return "bool"
	case "strs":
		return "[]string"
	case "nums":
		return "[]float64"
	case "nums2":
		return "[][]float64"
	case "rows":
		return "func(int) Element"
	}
	fail("no Go type for " + p.str("name"))
	return ""
}

// argType is the type as a method takes it: a list is variadic.
func argType(p table) string {
	switch p.str("type") {
	case "strs":
		return "...string"
	case "nums":
		return "...float64"
	}
	return goType(p)
}

// opParams is a drawing command's arguments as the Painter method takes
// them, every one explicit: Go has no optional argument. The `args`
// spec is read the way tools/gen reads it — `name: type`, or
// `name=pixname: type = default`.
func opParams(op table) []string {
	var params []string
	for _, a := range strings.Split(op.str("args"), ",") {
		spec, _, _ := strings.Cut(strings.TrimSpace(a), "=")
		spec = strings.TrimSpace(spec)
		an, at, _ := strings.Cut(spec, ":")
		an, at = strings.TrimSpace(an), strings.TrimSpace(at)
		if at == "" {
			full := strings.TrimSpace(a)
			_, rest, _ := strings.Cut(full, ":")
			at, _, _ = strings.Cut(strings.TrimSpace(rest), "=")
			at = strings.TrimSpace(at)
		}
		var gt string
		switch at {
		case "int":
			gt = "int"
		case "str":
			gt = "string"
		case "bool":
			gt = "bool"
		default:
			fail("no type for op arg " + a)
		}
		params = append(params, lower(an)+" "+gt)
	}
	return params
}

// The page shows what an app writes: a bool is `true` / `false`, a
// string is a Go literal, a list has no default to show.
func showDefault(p table) (string, bool) {
	d, ok := p["default"]
	if !ok {
		return "", false
	}
	switch v := d.(type) {
	case []any:
		return "—", true
	case bool:
		return "`" + strconv.FormatBool(v) + "`", true
	case string:
		return "`" + strconv.Quote(v) + "`", true
	case int64:
		return "`" + strconv.FormatInt(v, 10) + "`", true
	case float64:
		return "`" + strconv.FormatFloat(v, 'f', -1, 64) + "`", true
	}
	return "", false
}

// --- the code, which the page is held to ------------------------------------------------

// What elements.go declares: every top-level func by name, and every
// method by receiver type and name, each with its parameter list as
// written.
type code struct {
	funcs   map[string]string
	methods map[string]map[string]string
}

func readCode(path string) *code {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, path, nil, 0)
	if err != nil {
		fail(err.Error())
	}
	c := &code{funcs: map[string]string{}, methods: map[string]map[string]string{}}
	for _, d := range f.Decls {
		fn, ok := d.(*ast.FuncDecl)
		if !ok {
			continue
		}
		params := paramsText(fset, fn.Type)
		if fn.Recv == nil {
			c.funcs[fn.Name.Name] = params
			continue
		}
		t := fn.Recv.List[0].Type
		if star, ok := t.(*ast.StarExpr); ok {
			t = star.X
		}
		if idx, ok := t.(*ast.IndexExpr); ok {
			t = idx.X
		}
		id, ok := t.(*ast.Ident)
		if !ok {
			continue
		}
		if c.methods[id.Name] == nil {
			c.methods[id.Name] = map[string]string{}
		}
		c.methods[id.Name][fn.Name.Name] = params
	}
	return c
}

func paramsText(fset *token.FileSet, ft *ast.FuncType) string {
	var parts []string
	for _, f := range ft.Params.List {
		var buf bytes.Buffer
		printer.Fprint(&buf, fset, f.Type)
		t := buf.String()
		if len(f.Names) == 0 {
			parts = append(parts, t)
			continue
		}
		for _, n := range f.Names {
			parts = append(parts, n.Name+" "+t)
		}
	}
	return strings.Join(parts, ", ")
}

// fn is a constructor the page is about to list, held to the code.
func (c *code) fn(name, params string) string {
	got, ok := c.funcs[name]
	if !ok {
		fail("elements.go has no func " + name)
	}
	if got != params {
		fail(fmt.Sprintf("elements.go spells %s(%s), not %s(%s)", name, got, name, params))
	}
	return name + "(" + params + ")"
}

// method is a method the page is about to list, held to the code.
func (c *code) method(recv, name, params string) {
	got, ok := c.methods[recv][name]
	if !ok {
		fail(fmt.Sprintf("elements.go has no method %s on %s", name, recv))
	}
	if got != params {
		fail(fmt.Sprintf("elements.go spells %s.%s(%s), not %s.%s(%s)", recv, name, got, recv, name, params))
	}
}

// --- the words ------------------------------------------------------------------------

type words struct {
	title, intro            string
	ridersHead, ridersIntro string
	groups                  map[string]string
	th                      [3]string
	opTh                    [2]string
	written, children       string
	owns, ownsLabel, none   string
	opsHead, opsIntro       string
	footer                  string
}

var WORDS = map[string]words{
	"en": {
		title: "Elements",
		intro: "%d elements, and the %d methods every one of them has. This page is\n" +
			"written from `elements.toml`, the one table `go run ./tools/gen` writes\n" +
			"the Go an app calls from — `elements.go`, one type per element and one\n" +
			"method per keyword — together with the interpreter's view of the package\n" +
			"and the numbers both sides of the engine's C face count with. A method\n" +
			"that is not here is one an app cannot write, and Go itself says so: a\n" +
			"call to it is a type error, in `gomamochi check` and in `go build` alike.\n" +
			"\n" +
			"Types on this page are Go's: **string**, **float64**, **int** and\n" +
			"**bool**; a list of strings or of numbers is written as a variadic\n" +
			"argument, `...string` or `...float64`, a list of lists as `[][]float64`,\n" +
			"and the closure that builds row i as `func(int) Element`. A **handler**\n" +
			"is a closure handed to the method the element names for it, taking what\n" +
			"the event carries: `func()`, `func(string)`, `func(bool)`, `func(int)` or\n" +
			"`func(float64)`; see [Handlers](tour-logic.md#handlers).\n",
		ridersHead: "The methods every element has",
		ridersIntro: "These fifteen are methods on every element, under one name and one\n" +
			"meaning. They are written once, on the box every element is built over,\n" +
			"rather than repeated on thirty of them — which is why an element that\n" +
			"owns one of the names under its own meaning keeps it: a `Text`'s\n" +
			"`.Width` is the text's own method, and it shadows the shared one.\n",
		groups: map[string]string{
			"text":     "Text, buttons, links",
			"arrange":  "The boxes that arrange",
			"fields":   "Fields and choosers",
			"rows":     "Lists and tables",
			"charts":   "Charts and progress",
			"pictures": "Pictures and the canvas",
			"small":    "The small pieces",
		},
		th:        [3]string{"Method", "Type", "Default"},
		opTh:      [2]string{"Command", "Written as"},
		written:   "Written as `%s`.",
		children:  "Takes elements as its children, written as its last arguments.",
		owns:      "Sizes itself with its own %s: those are the element's, and the shared methods leave them alone.",
		ownsLabel: "Its own `label` is what a screen reader reads; `.A11yLabel` is refused on it, and stops the app with that reason.",
		none:      "No methods of its own.",
		opsHead:   "The canvas's drawing commands",
		opsIntro: "The %d commands below are methods on the `*Painter` a `Canvas`'s\n" +
			"`.Paint` closure is handed. They are not elements: they have none of the\n" +
			"methods above, nothing can click them, and they mean nothing outside the\n" +
			"canvas they are written in. Every coordinate is a whole virtual pixel\n" +
			"and every color is a number, the index of a color in the canvas's\n" +
			"palette. Go has no optional argument, so a value the table gives a\n" +
			"default to is written all the same.\n",
		footer: "## Adding one\n" +
			"\n" +
			"An element is a row in `elements.toml` and an arm in the engine's\n" +
			"`materialize`. `go run ./tools/gen` writes `elements.go` and\n" +
			"`internal/symbols/symbols.go` from it — one type per element, one method\n" +
			"per keyword, and the interpreter's view of the package — and the sweep\n" +
			"fails when either is behind the table, so an element cannot come to mean\n" +
			"one thing in Go and another where it is drawn. The same table is read by\n" +
			"the other three languages on this engine.\n",
	},
	"ja": {
		title: "要素",
		intro: "要素は %d 個、そのすべてが持つ共通のメソッドが %d 個あります。\n" +
			"このページは `elements.toml` から生成しています。\n" +
			"アプリが呼ぶ Go（`elements.go`。要素ごとの型と、キーワードごとのメソッド）も、解釈実行のインタプリタに見せる表も、エンジンの C API で両側が数える番号も、同じ表から `go run ./tools/gen` が書き出します。\n" +
			"ここにないメソッドは、アプリには書けません。\n" +
			"書いた場合は、Go 自身が型の誤りとして報告します。\n" +
			"`gomamochi check` でも `go build` でも同じです。\n" +
			"\n" +
			"このページでの型の読み方です。\n" +
			"型は Go のものをそのまま書きます。\n" +
			"**string**、**float64**、**int**、**bool** はそのままの意味です。\n" +
			"文字列や数のリストは可変長引数（`...string`、`...float64`）として書き、リストのリストは `[][]float64`、i 行目を作るクロージャは `func(int) Element` です。\n" +
			"**ハンドラ** は、要素がそのために用意したメソッドに渡すクロージャで、その出来事が運ぶものを受け取ります（`func()`、`func(string)`、`func(bool)`、`func(int)`、`func(float64)`）。\n" +
			"詳しくは[ハンドラ](tour-logic.md#ハンドラ)を見てください。\n",
		ridersHead: "すべての要素が持つメソッド",
		ridersIntro: "どの要素も、この 15 個を同じ名前と同じ意味で持ちます。\n" +
			"30 個の要素にそれぞれメソッドを足すのではなく、要素の土台になる一つの箱に一度だけ書いてあります。\n" +
			"同じ名前を要素自身が別の意味で持っている場合は、要素のものが優先されます。\n" +
			"`Text` の `.Width` は文字列そのものの幅で、共通のメソッドはそれに隠れます。\n",
		groups: map[string]string{
			"text":     "文字とボタンとリンク",
			"arrange":  "並べる箱",
			"fields":   "入力と選択",
			"rows":     "リストと表",
			"charts":   "グラフと進捗",
			"pictures": "画像とキャンバス",
			"small":    "小さな要素",
		},
		th:        [3]string{"メソッド", "型", "既定値"},
		opTh:      [2]string{"命令", "書き方"},
		written:   "`%s` と書きます。",
		children:  "要素を子に取ります。\n子は最後の引数として書きます。",
		owns:      "大きさは自身の %s で決めます。\n共通のメソッドは手を出しません。",
		ownsLabel: "自身の `label` が画面読み上げの読む名前なので、`.A11yLabel` はここでは受け取りません。\n呼ぶと、その理由を出してアプリが止まります。",
		none:      "固有のメソッドはありません。",
		opsHead:   "キャンバスの描画命令",
		opsIntro: "次の %d 個の命令は、`Canvas` の `.Paint` に渡すクロージャの中で、渡された `*Painter` のメソッドとして書きます。\n" +
			"これらは要素ではありません。\n" +
			"上のメソッドをどれも持たず、押すこともできず、書かれたキャンバスの外では意味を持ちません。\n" +
			"座標はすべて仮想的な画素の整数で、色はすべて番号です。\n" +
			"番号はそのキャンバスの配色の何番目か、というだけのものです。\n" +
			"Go に省略できる引数はないので、表に既定値のある値もすべて書きます。\n",
		footer: "## 要素を足すとき\n" +
			"\n" +
			"要素を足す作業は、`elements.toml` に 1 行足すことと、エンジンの `materialize` に分岐を 1 つ足すことです。\n" +
			"`go run ./tools/gen` が、表から `elements.go`（要素ごとの型と、キーワードごとのメソッド）と `internal/symbols/symbols.go`（解釈実行のインタプリタに見せる表）を書き出します。\n" +
			"どちらかが表より古ければ、`tools/gate_all.sh` が落ちます。\n" +
			"だから、ある要素が Go では 1 つの意味を持ち、描かれる側では別の意味を持つ、ということが起きません。\n" +
			"このエンジンの上にあるほかの三つの言語も、同じ表を読んでいます。\n",
	},
}

// --- the page -------------------------------------------------------------------------

func elementSection(el table, lang string, c *code) string {
	w := WORDS[lang]
	name := el.str("name")
	cn := camel(name)
	tn := cn + "El"

	// The constructor: the positional props in order, then the children.
	var params []string
	for _, p := range el.props() {
		if p.is("pos") {
			params = append(params, lower(p.str("name"))+" "+goType(p))
		}
	}
	if el.is("children") {
		params = append(params, "kids ...Element")
	}
	sig := c.fn(cn, strings.Join(params, ", "))

	out := []string{"### " + cn, "", el.str("doc"), "", fmt.Sprintf(w.written, sig), ""}
	if el.is("children") {
		out = append(out, w.children+"\n")
	}
	// A side the element sizes for itself is one of its own props: an
	// argument of the constructor when it is positional (a canvas's
	// width), a method of the element's own otherwise (a button's), and
	// the shared method of that name leaves it alone either way.
	var owned []string
	if native := el.str("native"); native != "" {
		for _, r := range riders {
			if !slices.Contains(strings.Split(r.str("owned"), ","), native) {
				continue
			}
			rn := r.str("name")
			own := propNamed(el, rn)
			if own == nil {
				fail(fmt.Sprintf("%s sizes its own %s, and has no such keyword", name, rn))
			}
			if own.is("pos") {
				owned = append(owned, "`"+lower(rn)+"`")
				continue
			}
			m := camel(rn)
			c.method(tn, m, "v "+argType(own))
			owned = append(owned, "`."+m+"`")
		}
	}
	if len(owned) > 0 {
		out = append(out, fmt.Sprintf(w.owns, strings.Join(owned, " / "))+"\n")
	}
	if el.is("owns_label") {
		if _, ok := c.methods[tn]["A11yLabel"]; !ok {
			fail("elements.go has no A11yLabel on " + tn)
		}
		out = append(out, w.ownsLabel+"\n")
	}

	// One row per method: every keyword that is not positional.
	var rows []string
	for _, p := range el.props() {
		if p.is("pos") {
			continue
		}
		m, t := camel(p.str("name")), argType(p)
		c.method(tn, m, "v "+t)
		def := "—"
		if !p.has("handler") {
			if d, ok := showDefault(p); ok {
				def = d
			}
		}
		rows = append(rows, fmt.Sprintf("| `.%s` | `%s` | %s |", m, t, def))
	}
	if el.is("paints") {
		c.method(tn, "Paint", "f func(*Painter)")
		rows = append(rows, "| `.Paint` | `func(*Painter)` | — |")
	}
	if len(rows) == 0 {
		out = append(out, w.none+"\n")
	} else {
		out = append(out, "| "+strings.Join(w.th[:], " | ")+" |", "|---|---|---|")
		out = append(out, rows...)
		out = append(out, "")
	}
	return strings.Join(out, "\n")
}

func opSection(lang string, c *code) string {
	w := WORDS[lang]
	out := []string{"## " + w.opsHead, "", strings.TrimRight(fmt.Sprintf(w.opsIntro, len(ops)), " \n"), "",
		"| " + strings.Join(w.opTh[:], " | ") + " |", "|---|---|"}
	for _, op := range ops {
		name := camel(op.str("name"))
		sig := strings.Join(opParams(op), ", ")
		c.method("Painter", name, sig)
		out = append(out, fmt.Sprintf("| `%s` | `p.%s(%s)` |", name, name, sig))
	}
	out = append(out, "")
	return strings.Join(out, "\n")
}

var manyNewlines = regexp.MustCompile(`\n{3,}`)

func pageText(lang string, c *code) string {
	w := WORDS[lang]
	out := []string{"<!-- Written by website/tools/elementspage from the table. Edit the table. -->",
		"# " + w.title, "", strings.TrimRight(fmt.Sprintf(w.intro, len(elements), len(riders)), " \n"), ""}
	out = append(out, "## "+w.ridersHead, "", strings.TrimRight(w.ridersIntro, " \n"), "",
		"| "+strings.Join(w.th[:], " | ")+" |", "|---|---|---|")
	for _, r := range riders {
		m := camel(r.str("name"))
		c.method("box", m, "v "+goType(r))
		def := "—"
		if !r.is("presence") {
			if d, ok := showDefault(r); ok {
				def = d
			}
		}
		out = append(out, fmt.Sprintf("| `.%s` | `%s` | %s |", m, goType(r), def))
	}
	out = append(out, "")
	for _, g := range groups {
		out = append(out, "## "+w.groups[g.key], "")
		for _, n := range g.names {
			out = append(out, elementSection(element[n], lang, c))
		}
	}
	out = append(out, opSection(lang, c))
	out = append(out, strings.TrimRight(w.footer, " \n"), "")
	return manyNewlines.ReplaceAllString(strings.Join(out, "\n"), "\n\n")
}

func main() {
	check := slices.Contains(os.Args[1:], "--check")
	dir := root()
	load(filepath.Join(dir, "..", "crates", "pixie-capi", "elements.toml"))
	c := readCode(filepath.Join(dir, "elements.go"))

	// Every element in the table is on the page, or the page is a lie;
	// and every name the page places is in the table.
	listed := map[string]bool{}
	for _, g := range groups {
		for _, n := range g.names {
			if _, ok := element[n]; !ok {
				fail("the page places an element the table does not have: " + n)
			}
			listed[n] = true
		}
	}
	var absent []string
	for _, e := range elements {
		if !listed[e.str("name")] {
			absent = append(absent, e.str("name"))
		}
	}
	if len(absent) > 0 {
		fail("the page has no place for: " + strings.Join(absent, ", "))
	}

	stale := 0
	for _, lang := range []string{"en", "ja"} {
		docs := "docs"
		if lang == "ja" {
			docs = "docs-ja"
		}
		rel := filepath.Join("website", docs, "elements.md")
		path := filepath.Join(dir, rel)
		text := pageText(lang, c)
		if check {
			have, _ := os.ReadFile(path)
			if string(have) != text {
				fmt.Fprintf(os.Stderr, "FAIL elements.md (%s) is behind the table\n", lang)
				stale++
			}
			continue
		}
		if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
			fail(err.Error())
		}
		fmt.Println("wrote", rel)
	}
	if stale > 0 {
		os.Exit(1)
	}
}
