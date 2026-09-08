// The site's Refusals page, in both languages, written from the fixtures.
//
// Every refusal under `test/refuse/` is a pair: an app Gomamochi cannot
// take, and the message `gomamochi check` must print for it, word for
// word. That message is the page, quoted from the file the sweep holds
// the checker to, so the page cannot describe a refusal in wording the
// command no longer uses. What is written here is the order, the
// grouping, and one line of why.
//
//	go run ./website/tools/refusalspage            write docs/refusals.md and docs-ja/
//	go run ./website/tools/refusalspage --check    fail if either is behind the fixtures
//
// Run from gomamochi/, the module root.
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"sort"
	"strings"
)

func fail(msg string) {
	fmt.Fprintln(os.Stderr, "refusalspage:", msg)
	os.Exit(1)
}

// The module root is where this runs from: go.mod, and website/ beside it.
func root() string {
	wd, err := os.Getwd()
	if err != nil {
		fail(err.Error())
	}
	if _, err := os.Stat(filepath.Join(wd, "go.mod")); err != nil {
		fail("run from gomamochi/: go run ./website/tools/refusalspage")
	}
	if st, err := os.Stat(filepath.Join(wd, "website")); err != nil || !st.IsDir() {
		fail("run from gomamochi/: go run ./website/tools/refusalspage")
	}
	return wd
}

var groups = []string{"shape", "interp", "views", "go"}

// group, fixture, the English line of why, the Japanese one.
type entry struct {
	group, name, en, ja string
}

var catalogue = []entry{
	{"shape", "package_not_main",
		"An app is a program: `package main`, with a `main` that hands the app to `Run`.",
		"アプリはプログラムです。\n`package main` で、`main` がアプリを `Run` に渡します。"},
	{"shape", "no_main",
		"Without a `main` there is nothing to run, and nothing to hand to `Run`.",
		"`main` がなければ、動かすものも `Run` に渡すものもありません。"},
	{"shape", "run_from_call",
		"The interpreter hands a call's result over without the wrapper that lets an interpreted type satisfy a compiled interface, and `Run` then cannot take it; a value with a name gets the wrapper.",
		"解釈実行の型がパッケージの `App` インタフェースの代わりを務めるにはラッパーが要りますが、呼び出しの結果をそのまま渡すと、インタプリタはラッパーなしで渡してしまいます。\nいったん変数に受けてから渡せばラッパーが付きます。"},

	{"interp", "min_max",
		"gc knows `min` and `max`; the interpreter, whose Go is 1.22's, does not, so the app would run one way and not the other.",
		"gc は `min` と `max` を知っていますが、Go 1.22 相当のインタプリタは知りません。\nそのままでは、片方の実行でしか動かないアプリになります。"},
	{"interp", "range_number",
		"`range` over a number is Go 1.22's; the interpreter stops on it rather than answering something else, so it is refused before it runs.",
		"数に対する `range` は Go 1.22 の書き方で、インタプリタは別の答えを返すのではなくそこで止まります。\nだから走らせる前に断ります。"},
	{"interp", "range_count",
		"The same shape with a variable rather than a literal, which only the type checker can see; it is refused when `go` is on the path.",
		"同じ書き方を、リテラルではなく変数で書いたものです。\n型を検査しなければ見つからないので、`go` がパスにあるときに断ります。"},
	{"interp", "range_function",
		"`range` over a function is Go 1.23's, and the interpreter stops on it too.",
		"関数に対する `range` は Go 1.23 の書き方で、インタプリタはこれにも止まります。"},
	{"interp", "percent_t",
		"`%T` prints the interpreter's name for an app's type, not Go's, so the two runs would print different text.",
		"`%T` が出力するのは、アプリの型をインタプリタが呼ぶ名前で、Go の名前ではありません。\n二つの実行が違う文字列を出力することになります。"},
	{"interp", "import_reflect",
		"`reflect` sees the same difference: the interpreter names the app's own types differently from the compiled run.",
		"`reflect` からも同じ違いが見えます。\nインタプリタは、アプリ自身の型をコンパイルした実行とは別の名前で呼びます。"},
	{"interp", "import_unsafe",
		"The interpreted run does not take `unsafe`.",
		"解釈実行は `unsafe` を扱えません。"},
	{"interp", "import_cgo",
		"The interpreted run cannot call C, and the compiled run is built without cgo.",
		"解釈実行は C を呼べません。\nコンパイルした実行も cgo なしでビルドします。"},
	{"interp", "import_embed",
		"The interpreted run reads the file as source, and has nothing to embed.",
		"解釈実行はファイルをソースとして読むので、埋め込むものがありません。"},
	{"interp", "import_module",
		"The interpreted run cannot read a module outside the standard library yet; the standard library and this package are what an app imports.",
		"解釈実行は、標準ライブラリの外のモジュールをまだ読めません。\nアプリが import できるのは標準ライブラリとこのパッケージです。"},
	{"interp", "loop_variable_written",
		"The interpreted run gets a per-iteration copy of a captured loop variable; a body that writes the variable would be writing something the copy hides.",
		"解釈実行では、クロージャが捕まえたループ変数は反復ごとのコピーになります。\n本体でその変数に代入しても、クロージャが見るのはコピーのほうです。"},

	{"views", "view_write",
		"Building a screen twice has to build the same screen, so building it only reads.",
		"同じ画面を二度組み立てたら同じ画面になる必要があるので、組み立てるときは読むだけです。"},
	{"views", "view_goroutine",
		"A view is built again whenever anything changes, and each build would start the work again; a handler starts it once, with `Task`.",
		"ビューは何かが変わるたびに組み立て直されるので、そのたびに処理が走り出してしまいます。\nハンドラから `Task` で一度だけ始めます。"},
	{"views", "view_clock",
		"The clock answers differently from one build to the next, and the two runs would draw different screens; a timer reads it and keeps the answer on the app.",
		"時計は組み立てるたびに違う値を返し、二つの実行が違う画面を描くことになります。\nタイマーで読んで、その値をアプリに持たせます。"},
	{"views", "view_environment",
		"The environment, a file, a stream, the network and a random number are read in a handler, and what they answered is kept on the app.",
		"環境変数、ファイル、ストリーム、ネットワーク、乱数はハンドラで読み、読んだ値をアプリに持たせます。"},
	{"views", "view_keyboard",
		"The keyboard is a device, read from a timer and never from a view.",
		"キーボードはデバイスで、読むのはタイマーからです。\nビューからは読みません。"},
	{"views", "range_map",
		"A map walks in a different order every run, and both runs would draw different screens; a handler may range over one to collect and sort its keys.",
		"マップを `range` で回る順序は走らせるたびに変わり、二つの実行が違う画面を描くことになります。\nキーを集めて並べ替えるだけなら、ハンドラの中で回せます。"},

	{"go", "undefined_name",
		"When nothing here has anything to say, Go's own type checker speaks, in Go's own words.",
		"Gomamochi の側に言うことがなければ、Go 自身の型検査が、Go 自身の言葉で誤りを伝えます。"},
}

type words struct {
	title  string
	intro  string
	groups map[string]string
}

var WORDS = map[string]words{
	"en": {
		title: "What Gomamochi refuses",
		intro: "There is no translator in Gomamochi, so there is no second language to\n" +
			"learn: an app is Go. What `gomamochi check` refuses is the Go the\n" +
			"interpreted run cannot run the way the compiled one does, and the few\n" +
			"rules a view has to keep. It reads the app and names what it cannot\n" +
			"take, with the file, the line and the column, the line itself, and what\n" +
			"to write instead; with `go` on the path it also type-checks the file the\n" +
			"way the compiler will, and Go's own errors come back in Go's own words.\n" +
			"`check` runs before every build and every gate, builds nothing and opens\n" +
			"no window, and prints nothing at all when there is nothing to say.\n" +
			"\n" +
			"Each of the %d below has a file under `test/refuse/` that triggers it and\n" +
			"the message it must print, word for word. The sweep runs them, so a\n" +
			"refusal cannot quietly change its wording, and this page is quoted from\n" +
			"those same files.\n",
		groups: map[string]string{
			"shape":  "The app's shape",
			"interp": "What the interpreted run cannot run the way the compiler does",
			"views":  "Views",
			"go":     "Go's own verdict",
		},
	},
	"ja": {
		title: "Gomamochi が断る書き方",
		intro: "Gomamochi に翻訳器はないので、覚える二つ目の言語もありません。\n" +
			"アプリは Go です。\n" +
			"`gomamochi check` が断るのは、解釈実行がコンパイルした実行と同じには動かせない Go と、ビューが守る少数の規則です。\n" +
			"アプリを読み、受け取れない書き方があれば、ファイルと行と桁、その行そのもの、そして代わりの書き方を示します。\n" +
			"`go` がパスにあれば、コンパイラと同じように型も検査します。\n" +
			"そこで見つかった誤りは、Go 自身の言葉のまま出ます。\n" +
			"`check` はビルドの前にもゲートの前にも走ります。\n" +
			"何もビルドせず、ウィンドウも開かず、言うことがなければ何も出力しません。\n" +
			"\n" +
			"下の %d 個には、それを起こすファイルと、出力されるべき文面が、`test/refuse/` にそのまま置いてあります。\n" +
			"`tools/gate_all.sh` がそれを回すので、断りの文面が黙って変わることはありません。\n" +
			"このページも、その同じファイルから引いています。\n",
		groups: map[string]string{
			"shape":  "アプリの形",
			"interp": "解釈実行がコンパイルした実行と同じには動かせない書き方",
			"views":  "ビュー",
			"go":     "Go 自身の判定",
		},
	},
}

var fix string

func fixture(name string) string {
	text, err := os.ReadFile(filepath.Join(fix, name+".txt"))
	if err != nil {
		fail(err.Error())
	}
	return strings.TrimRight(string(text), " \t\r\n")
}

// have is every fixture, by name, in sorted order.
func have() []string {
	entries, err := os.ReadDir(fix)
	if err != nil {
		fail(err.Error())
	}
	var names []string
	for _, e := range entries {
		if n := e.Name(); strings.HasSuffix(n, ".txt") && !e.IsDir() {
			names = append(names, strings.TrimSuffix(n, ".txt"))
		}
	}
	sort.Strings(names)
	return names
}

var manyNewlines = regexp.MustCompile(`\n{3,}`)

func pageText(lang string, n int) string {
	w := WORDS[lang]
	out := []string{"<!-- Written by website/tools/refusalspage from test/refuse/. Edit the fixtures. -->",
		"# " + w.title, "", strings.TrimRight(fmt.Sprintf(w.intro, n), " \n"), ""}
	for _, group := range groups {
		var in []entry
		for _, e := range catalogue {
			if e.group == group {
				in = append(in, e)
			}
		}
		if len(in) == 0 {
			continue
		}
		out = append(out, "## "+w.groups[group], "")
		for _, e := range in {
			why := e.en
			if lang == "ja" {
				why = e.ja
			}
			out = append(out, why, "")
			out = append(out, "```console", fixture(e.name), "```", "")
		}
	}
	return manyNewlines.ReplaceAllString(strings.Join(out, "\n"), "\n\n")
}

func main() {
	check := slices.Contains(os.Args[1:], "--check")
	dir := root()
	fix = filepath.Join(dir, "test", "refuse")

	// Every fixture is on the page, or the page is not the whole of it.
	names := have()
	listed := map[string]bool{}
	for _, e := range catalogue {
		listed[e.name] = true
	}
	var absent []string
	for _, n := range names {
		if !listed[n] {
			absent = append(absent, n)
		}
	}
	if len(absent) > 0 {
		fail("the page has no entry for: " + strings.Join(absent, " "))
	}
	var gone []string
	for _, e := range catalogue {
		if !slices.Contains(names, e.name) {
			gone = append(gone, e.name)
		}
	}
	if len(gone) > 0 {
		fail("the page lists refusals that have no fixture: " + strings.Join(gone, ", "))
	}

	stale := 0
	for _, lang := range []string{"en", "ja"} {
		docs := "docs"
		if lang == "ja" {
			docs = "docs-ja"
		}
		rel := filepath.Join("website", docs, "refusals.md")
		path := filepath.Join(dir, rel)
		text := pageText(lang, len(names))
		if check {
			have, _ := os.ReadFile(path)
			if string(have) != text {
				fmt.Fprintf(os.Stderr, "FAIL refusals.md (%s) is behind the fixtures\n", lang)
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
