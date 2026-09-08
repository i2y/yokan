// The site's Demos page, in both languages, written from demo/.
//
// A gallery is a page of usage samples, so the sample has to be on it:
// each demo gets its screenshot and, under it, its whole source in a
// collapsed block. The gloss for each one is written here, once per
// language, and a demo with no entry stops this program, so a new demo
// cannot quietly go missing from the gallery.
//
//	go run ./website/tools/demospage            write docs/demos.md and docs-ja/
//	go run ./website/tools/demospage --check    fail if either is behind demo/
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
	fmt.Fprintln(os.Stderr, "demospage:", msg)
	os.Exit(1)
}

// The module root is where this runs from: go.mod, and website/ beside it.
func root() string {
	wd, err := os.Getwd()
	if err != nil {
		fail(err.Error())
	}
	if _, err := os.Stat(filepath.Join(wd, "go.mod")); err != nil {
		fail("run from gomamochi/: go run ./website/tools/demospage")
	}
	if st, err := os.Stat(filepath.Join(wd, "website")); err != nil || !st.IsDir() {
		fail("run from gomamochi/: go run ./website/tools/demospage")
	}
	return wd
}

var groups = []string{"start", "state", "look", "lists", "canvas", "go", "window", "work"}

// group, name, the English gloss, the Japanese one.
type entry struct {
	group, name, en, ja string
}

var catalogue = []entry{
	{"start", "counter",
		"the reference: an app is a struct, its state is its fields, and a handler is a closure over them",
		"基本形。アプリは構造体で、状態はそのフィールド、ハンドラはフィールドがそのまま見えるクロージャ"},
	{"start", "prefixed",
		"the counter again with the package imported under a name: every call reads `gm.`, and the app may use any name for its own types",
		"同じカウンタを、パッケージに名前を付けて import した書き方で。呼び出しはすべて `gm.` で始まり、アプリ自身の型にはどんな名前を付けてもよい"},
	{"start", "control",
		"ordinary Go inside a view: `if`, a loop, and a method that answers part of the screen; the view builds a slice of elements the way any Go does",
		"ビューの中はただの Go。`if`、ループ、画面の一部を返すメソッド。要素のスライスは、普通の Go と同じ書き方で組み立てる"},
	{"start", "todo",
		"a list whose rows are built on demand, and a field that submits with enter",
		"行を必要なぶんだけ作るリストと、enter で確定する入力欄"},
	{"start", "calc",
		"a calculator: one accumulator, one pending operation, and a look that is a function setting the same properties on every key",
		"電卓。途中の値ひとつと待っている演算ひとつを持ち、見た目は、どのキーにも同じメソッドを並べる関数にまとめる"},
	{"start", "calcgrid",
		"the same calculator on a grid instead of five rows; `ColSpan` is what makes the zero key twice as wide",
		"同じ電卓を 5 行ではなくグリッドで。0 キーが 2 つぶんの幅になるのは `ColSpan`"},

	{"state", "mixer",
		"state an app keeps, and a field that writes into it",
		"アプリが持つ状態と、そこへ書き込む入力欄"},
	{"state", "lookup",
		"a map on the app: read with a fallback, asked whether a key is there, added to while the window is open, and never ranged over in a view",
		"アプリが持つマップ。既定値つきの読み出し、キーがあるかどうかの確認、ウィンドウを開けたままの追加。ビューの中でマップを走査することはない"},
	{"state", "points",
		"a small struct of values, carried on the app's own state",
		"値のための小さな構造体を、アプリの状態として持つ"},
	{"state", "moods",
		"values that are one of a few named things (constants) and a value that may be nothing (a pointer that may be nil)",
		"決まった名前のどれか一つになる値（定数）と、何も入っていないかもしれない値（nil になりうるポインタ）"},
	{"state", "links",
		"objects that point at one another; Go's collector takes a cycle in its stride, so the back pointer is an ordinary pointer",
		"互いを指し合うオブジェクト。Go のガベージコレクタは循環参照も回収するので、親を指すフィールドも普通のポインタでよい"},

	{"look", "forms",
		"the controls a person changes: a box, a switch, a track, and the four choosers",
		"人が動かす要素。チェックボックス、スイッチ、スライダ、そして 4 種類の選択"},
	{"look", "quantities",
		"the two fields that hold a number rather than text: enter or leaving the field commits, and text that is not a number is dropped",
		"文字ではなく数を持つ 2 つの入力欄。enter か、欄を離れることで確定し、数でない文字は捨てられる"},
	{"look", "layout",
		"spacer and divider: a filler that pushes what follows to the edge, and a rule",
		"Spacer と Divider。続きを端まで押しやる余白と、区切り線"},
	{"look", "cards",
		"a piece of screen with a name is a method, and one that wraps other elements takes them as arguments",
		"名前のついた画面の一部はメソッド。ほかの要素を包むものは、それらを引数に取る"},
	{"look", "styled",
		"a look kept in one place: a function that sets the same properties on every button, and `Theme` flipping a whole panel",
		"見た目をひとところに。どのボタンにも同じメソッドを並べる関数と、パネルを丸ごと切り替える `Theme`"},
	{"look", "badges",
		"text as a pill, and the rest of what a run of text can be: monospace, underlined, italic, clipped, clamped",
		"文字を丸いラベルに。等幅、下線、斜体、省略記号での打ち切り、行数の制限"},
	{"look", "panels",
		"the elements that arrange or cover: tracks, layers, panes that scroll, and a panel over the rest of the window",
		"並べる要素と覆う要素。グリッド、重ね、スクロールする面、ウィンドウの上に出る一枚"},
	{"look", "dialog",
		"a panel over the rest of the window, opened and closed by the app",
		"ウィンドウの上に出る一枚を、アプリが開いて閉じる"},
	{"look", "labels",
		"what a screen reader is told and what the pointer shows; `Role` takes a value, so a line is a heading until it is not",
		"画面読み上げに伝える名前と、ポインタが見せる説明。`Role` は値を取るので、行が見出しであるかどうかを切り替えられる"},
	{"look", "shared",
		"the methods every element has, on elements that have nothing else in common",
		"共通のメソッドを、種類の違う要素それぞれに付けてみる"},
	{"look", "loading",
		"the bar that fills, in its three forms: with a caption, at a size the app chose, and sweeping for work with no known length",
		"満ちていくバーの 3 つの形。見出しつき、アプリが決めた大きさ、そして終わりの見えない処理のために行き来する表示"},
	{"look", "filter",
		"a chooser that changes what a list shows, with the rows built on demand",
		"リストが見せるものを変える選択。行は必要なぶんだけ作られる"},

	{"lists", "table",
		"`DataTable` draws the table itself: the first row is the header, the later ones are shaded in alternation",
		"`DataTable` が表そのものを描く。最初の行が見出しで、以降は交互に色の変わるデータ行"},
	{"lists", "roster",
		"the table that builds its rows on demand, with row selection and header sort the app performs itself",
		"行を必要なぶんだけ作る表。行の選択と見出しでの並べ替えは、アプリ自身が行う"},
	{"lists", "csv_viewer",
		"a hundred thousand rows, filtered as you type; only the rows in the window are ever built",
		"10 万行を、打ちながら絞り込む。作られるのは画面に入っている行だけ"},
	{"lists", "trend",
		"one list of numbers, drawn twice",
		"ひとつの数のリストを、2 通りに描く"},
	{"lists", "charts",
		"losses below the zero line, a pinned range, an axis with gridlines, and two series with their own colors",
		"0 の線より下に伸びる損失、固定した範囲、目盛りと補助線のある軸、色を持つ 2 本の系列"},

	{"canvas", "canvas",
		"a grid of virtual pixels painted command by command, colors by palette index, and the keyboard read from the tick",
		"仮想的な画素の格子を、命令をひとつずつ並べて塗る。色は配色の番号、キーボードはタイマーから読む"},
	{"canvas", "jump",
		"Pyxel's jump game, ported: gravity, floors that fall away when you land on them, fruit, and scenery scrolling at its own speed",
		"Pyxel のジャンプゲームの移植。重力、乗ると落ちる床、果物、それぞれの速さで流れる背景"},
	{"canvas", "shooter",
		"Pyxel's shoot-'em-up, ported: scenes, parallax stars, enemies that sway as they fall, collisions and expanding blasts",
		"Pyxel のシューティングの移植。場面の切り替え、視差のある星、揺れながら落ちてくる敵、当たり判定と広がる爆発"},

	{"go", "stdlib",
		"Go's own under the gate: `math`, `sort`, `time`, `encoding/json`, `encoding/csv`, `strings`, `regexp` — the same compiled packages in both runs",
		"Go 自身のものをゲートにかける。`math`、`sort`、`time`、`encoding/json`、`encoding/csv`、`strings`、`regexp`。両方の実行が同じコンパイル済みのパッケージを呼ぶ"},
	{"go", "files",
		"files with Go's own `os`; nothing here is Gomamochi's, and the gate says both runs answer the same",
		"Go 自身の `os` でファイルを扱う。Gomamochi 固有のものは使わず、両方の実行が一致することはゲートが確かめる"},
	{"go", "reader",
		"a page fetched off the window's thread from a server the app runs itself, so both runs read the same bytes",
		"ウィンドウのスレッドの外で、アプリ自身が立てたサーバからページを取ってくる。サーバが自前なので、両方の実行が同じバイト列を読む"},
	{"go", "dbnotes",
		"a database reached through the engine, one implementation for both runs, with the values bound rather than spliced",
		"エンジン越しに触るデータベース。実装は一つで、両方の実行が同じものを呼ぶ。値は文に埋め込まず、束縛して渡す"},
	{"go", "ledger",
		"money kept in a database: an item called o'brien is an apostrophe and never a piece of SQL",
		"データベースに置いた家計簿。o'brien という品目はアポストロフィであって、SQL の一部にはならない"},
	{"go", "edges",
		"an index past the end stops the program in both runs, so the app asks the length first; a number past a machine word is `math/big`, the same package in both",
		"端の話。終わりを越えた添字はどちらの実行でもプログラムを止めるので、アプリは先に長さを確かめる。64 ビットに収まらない数は `math/big` で、これも両方の実行が同じパッケージを呼ぶ"},
	{"go", "flow",
		"control flow in the handlers: a loop that skips, a loop that stops, a `for` with a condition, and a method that wraps another",
		"ハンドラの中の制御構造。飛ばすループ、止まるループ、条件つきの `for`、別のメソッドを包むメソッド"},

	{"window", "keys",
		"the keyboard as chords and the same handlers in the menu bar, driven with `key:cmd+s` and `menu:Save`",
		"キーの組み合わせにハンドラを結び付け、同じものをメニューバーにも置く。`key:cmd+s` と `menu:Save` で動かせる"},
	{"window", "picker",
		"the platform's own file panels, asked for off the window's thread, and a file dragged onto the window",
		"OS 自身のファイル選択と、ウィンドウへ落とされたファイル。選択は人を待つので、ウィンドウのスレッドの外で頼む"},
	{"window", "about",
		"links that open a page, and the system clipboard",
		"ページを開くリンクと、システムのクリップボード"},
	{"window", "sound",
		"a WAV file played from a handler; a run under a script is silent, so the gate compares two silent runs",
		"ハンドラから WAV ファイルを鳴らす。スクリプトの下では無音になるので、ゲートが突き合わせるのは画面だけ"},

	{"work", "dashboard",
		"a timer declared before the app runs, ticking in both runs (the gate steps it with `advance:`)",
		"アプリを走らせる前に宣言するタイマー。両方の実行で同じだけ時を刻む（ゲートは `advance:` で進める）"},
	{"work", "tasks",
		"work that takes a while, done off the window's thread on a goroutine of its own; the answer comes back through the second closure",
		"時間のかかる処理を、専用の goroutine でウィンドウのスレッドの外へ。答えは二つ目のクロージャで受け取る"},
}

type words struct {
	title  string
	intro  string
	groups map[string]string
}

var WORDS = map[string]words{
	"en": {
		title: "Demos",
		intro: "%d apps, every one of them gated: the interpreted run and the compiled\n" +
			"one, driven by the same script, compared byte for byte. Each runs as-is\n" +
			"from `gomamochi/` in the repository.\n" +
			"\n" +
			"```console\n" +
			"$ ./bin/gomamochi run demo/counter.go    # substitute any demo's name\n" +
			"$ ./tools/gate_all.sh                    # gate every demo at once\n" +
			"```\n" +
			"\n" +
			"Every screenshot shows the state right after launch, except the two\n" +
			"games, which show a recording of play. The source under each one is the\n" +
			"whole file.\n",
		groups: map[string]string{
			"start":  "Start here",
			"state":  "State",
			"look":   "Look and layout",
			"lists":  "Lists, tables, charts",
			"canvas": "The canvas",
			"go":     "Go, files, data",
			"window": "The window",
			"work":   "Timers and work",
		},
	},
	"ja": {
		title: "デモ",
		intro: "アプリが %d 本あり、すべてがゲートを通っています。\n" +
			"解釈実行とコンパイルした実行を同じスクリプトで動かし、1 バイトずつ突き合わせています。\n" +
			"どれもリポジトリの `gomamochi/` からそのまま動きます。\n" +
			"\n" +
			"```console\n" +
			"$ ./bin/gomamochi run demo/counter.go    # 名前はどのデモでもよい\n" +
			"$ ./tools/gate_all.sh                    # すべてのデモをまとめてゲートにかける\n" +
			"```\n" +
			"\n" +
			"画面写真はどれも起動直後の状態です。\n" +
			"ただし 2 つのゲームだけは、遊んでいるところの録画です。\n" +
			"その下にあるのは、そのデモのファイル全体です。\n",
		groups: map[string]string{
			"start":  "まずはここから",
			"state":  "状態",
			"look":   "見た目と配置",
			"lists":  "リストと表とグラフ",
			"canvas": "キャンバス",
			"go":     "Go とファイルとデータ",
			"window": "ウィンドウまわり",
			"work":   "タイマーとタスク",
		},
	},
}

// --- the tree the page claims things about -----------------------------------

var demos, shots string

// apps is every demo, by name, in sorted order.
func apps() []string {
	entries, err := os.ReadDir(demos)
	if err != nil {
		fail(err.Error())
	}
	var names []string
	for _, e := range entries {
		if n := e.Name(); strings.HasSuffix(n, ".go") && !e.IsDir() {
			names = append(names, strings.TrimSuffix(n, ".go"))
		}
	}
	sort.Strings(names)
	return names
}

// shot is the picture of a demo: a recording of play where there is
// one, the first screen otherwise.
func shot(name string) string {
	for _, ext := range []string{"gif", "png"} {
		if _, err := os.Stat(filepath.Join(shots, name+"."+ext)); err == nil {
			return name + "." + ext
		}
	}
	fail("demo/screenshots has no picture of " + name)
	return ""
}

func source(name string) string {
	text, err := os.ReadFile(filepath.Join(demos, name+".go"))
	if err != nil {
		fail(err.Error())
	}
	return strings.TrimRight(string(text), " \t\r\n")
}

var manyNewlines = regexp.MustCompile(`\n{3,}`)

func pageText(lang string, n int) string {
	w := WORDS[lang]
	out := []string{"<!-- Written by website/tools/demospage from demo/. Edit the demos. -->",
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
			gloss := e.en
			if lang == "ja" {
				gloss = e.ja
			}
			pic := shot(e.name)
			width := 360
			if strings.HasSuffix(pic, ".gif") {
				width = 300
			}
			out = append(out, "#### "+e.name+" — "+gloss)
			out = append(out, fmt.Sprintf(`<img src="images/demos/%s" width="%d">`, pic, width), "")
			out = append(out, fmt.Sprintf(`??? note "%s.go"`, e.name), "")
			out = append(out, "    ```go")
			for _, line := range strings.Split(source(e.name), "\n") {
				if line == "" {
					out = append(out, "")
				} else {
					out = append(out, "    "+line)
				}
			}
			out = append(out, "    ```", "")
		}
	}
	text := manyNewlines.ReplaceAllString(strings.Join(out, "\n"), "\n\n")
	return text + "\n"
}

func main() {
	check := slices.Contains(os.Args[1:], "--check")
	dir := root()
	demos = filepath.Join(dir, "demo")
	shots = filepath.Join(demos, "screenshots")

	// Every demo has an entry, and every entry a demo.
	names := apps()
	listed := map[string]bool{}
	for _, e := range catalogue {
		listed[e.name] = true
	}
	var missing []string
	for _, n := range names {
		if !listed[n] {
			missing = append(missing, n)
		}
	}
	if len(missing) > 0 {
		fail("demo/ has apps the gallery has no entry for: " + strings.Join(missing, " "))
	}
	var gone []string
	for _, e := range catalogue {
		if !slices.Contains(names, e.name) {
			gone = append(gone, e.name)
		}
	}
	if len(gone) > 0 {
		fail("the gallery lists apps demo/ does not have: " + strings.Join(gone, ", "))
	}

	stale := 0
	for _, lang := range []string{"en", "ja"} {
		docs := "docs"
		if lang == "ja" {
			docs = "docs-ja"
		}
		rel := filepath.Join("website", docs, "demos.md")
		path := filepath.Join(dir, rel)
		text := pageText(lang, len(names))
		if check {
			have, _ := os.ReadFile(path)
			if string(have) != text {
				fmt.Fprintf(os.Stderr, "FAIL demos.md (%s) is behind demo/\n", lang)
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
