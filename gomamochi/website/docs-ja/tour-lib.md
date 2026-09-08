<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# Go とデータとタスク

Go 自身のライブラリ、フレームワークのライブラリ、タイマー、ウィンドウの外でする処理、そして保存すると書き換わるウィンドウ。

## Go 自身の標準ライブラリ

`fmt`、`strings`、`strconv`、`sort`、`math`、`time`、`encoding/json`、`encoding/csv`、`regexp`、`os`、`net/http` は言語自身のもので、両方の実行が同じコンパイル済みのパッケージを呼びます。
インタプリタはそれらを実装し直しません。
呼ぶのはコマンドに組み込まれたパッケージそのものです。
だから `fmt.Sprintf("%.2f", x)` が返すバイト列は、両方の実行で同じです。
ゲートがライブラリの二つの実装を比べることもありません。

<!-- script: click:stats,click:parse,click:scan,dump -->
```go
package main

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strconv"

	. "github.com/i2y/yokan/gomamochi"
)

type Stdlib struct {
	scores []int
	spread string
	doc    string
	sum    int
}

func (s *Stdlib) stats() {
	sorted := append([]int{}, s.scores...)
	sort.Ints(sorted)
	s.spread = fmt.Sprintf("median %d min %d max %d", sorted[len(sorted)/2], sorted[0], sorted[len(sorted)-1])
}

func (s *Stdlib) parse() {
	var doc map[string]any
	json.Unmarshal([]byte(`{"name": "gomamochi", "ok": true}`), &doc)
	s.doc = fmt.Sprintf("%v %v", doc["name"], doc["ok"])
}

func (s *Stdlib) scan() {
	s.sum = 0
	for _, m := range regexp.MustCompile(`\d+`).FindAllString("a1b22c333", -1) {
		v, _ := strconv.Atoi(m)
		s.sum += v
	}
}

func (s *Stdlib) View() Element {
	return Column(
		Text("spread: "+s.spread),
		Text("json: "+s.doc),
		Text(fmt.Sprintf("scan: %d", s.sum)),
		Row(
			Button("stats").OnClick(func() { s.stats() }),
			Button("parse").OnClick(func() { s.parse() }),
			Button("scan").OnClick(func() { s.scan() }),
		).Spacing(6),
	).Spacing(6).Padding(14)
}

func main() {
	Run(&Stdlib{scores: []int{3, 5, 8, 13, 21}, spread: "-", doc: "-"}, Title("stdlib"))
}
```

インタプリタが知っているライブラリは Go 1.22 のもので、そのリリースにあるパッケージと関数です。
言語のほうは、それより手前までしか知りません。
`min` と `max`（1.21）も、数を渡す `range`（1.22）も、関数を渡す `range`（1.23）もありません。
1.23 以降にライブラリへ足されたものも使えません。
最初の三つは、`check` がその名前を挙げて知らせます。
ファイルは `os`、ネットワークは `net/http`、大きな数は `math/big` です。
デモはファイルを読み、自分でページを立てて取得し、CSV を解析しますが、使うのは標準ライブラリだけです。


## フレームワークの標準ライブラリ

エンジンが仲立ちするものは、フレームワークのパッケージから来ます。
そこでは一つの実装が、同じ C API を通して両方の実行に答えます。
スクリプトで走らせるときは、どちらの実行でもダイアログに `file:<path>` の手順が答え、音は鳴りません。

| 関数 | 働き |
|---|---|
| `SqliteExec(db, sql, params...)` | 文を実行し、変わった行数を返す |
| `SqliteQueryText`, `SqliteQueryInt`, `SqliteQueryRows` | 問い合わせの最初の列、最初のセル、行全体 |
| `SqliteQueryTextOr`, `SqliteQueryIntOr`, `SqliteQueryRowsOr` | 同じもの。問い合わせが失敗しても止まらず、何も返さない |
| `ClipboardSetText(s)`, `ClipboardGetText()` | システムのクリップボード |
| `OpenDialog(title)`, `SaveDialog(name)` | プラットフォーム自身のパネル。人の操作を待つので、`Task` の中で呼ぶ |
| `AudioPlay(path, volume)`, `AudioStop()` | WAV ファイルを鳴らして、あとは放っておく |
| `NotifySend(title, body)` | 通知 |

文には `?` を書き、値はそのあとの引数として渡します。
そうすれば、人が打った文字が文の一部になることはありません。

<!-- script: click:setup,click:load,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

const db = "demo/.gate/tour-notes.db"

type Notes struct {
	changed int
	rows    []string
}

func (nt *Notes) setup() {
	SqliteExec(db, "CREATE TABLE IF NOT EXISTS notes(t TEXT)")
	SqliteExec(db, "DELETE FROM notes")
	nt.changed = SqliteExec(db, "INSERT INTO notes VALUES (?), (?)", "alpha", "beta")
}

func (nt *Notes) View() Element {
	return Column(
		Text(fmt.Sprintf("inserted=%d rows=%d", nt.changed, len(nt.rows))),
		Row(
			Button("setup").OnClick(func() { nt.setup() }),
			Button("load").OnClick(func() { nt.rows = SqliteQueryText(db, "SELECT t FROM notes ORDER BY t") }),
		).Spacing(6),
		ListView(len(nt.rows), func(i int) Element { return Text(nt.rows[i]) }).ItemHeight(22).Height(80),
	).Spacing(8).Padding(12)
}

func main() {
	Run(&Notes{}, Title("notes"))
}
```

テーブルができる前に走るかもしれない問い合わせは、何も返さない `…Or` の形で書きます。
帳簿の最初の読み込みがそうです。
素の形はアプリを止めます。
ライブラリ自身がそうするのと同じです。


## タイマーと、ウィンドウの外でする処理

決まった間隔で呼んでほしい処理は、`Every(seconds, tick)` の `tick` に渡します。
書くのは `Run` より前です。
二つの実行は一つの時計で動きます。
その時計を進めるのは、ウィンドウでは 1 フレーム、スクリプトでは `advance:<ms>` の 1 手順です。
だから、どちらの実行にも同じ数のティックが届きます。
ゲームが動くのも、キーボードを読むのも、このティックの中です。

`Task(work, done)` は、`work` を専用の goroutine で走らせます。
`work` が終わると、それが返した値を渡して、`done` をウィンドウのスレッドで呼びます。
`work` の中では、アプリのフィールドにも画面にも触りません。
決まりはこれだけです。
結果をどこかに書き込むのではなく `done` の引数で受け取るのも、そのためです。

<!-- script: click:start,dump -->
```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Jobs struct {
	status string
	answer int
}

func (j *Jobs) start() {
	j.status = "working"
	Task(func() any {
		total := 0
		for i := 0; i < 300000; i++ {
			total += i % 7
		}
		return total
	}, func(v any) {
		j.answer = v.(int)
		j.status = "done"
	})
}

func (j *Jobs) View() Element {
	return Column(
		Text(fmt.Sprintf("%s: %d", j.status, j.answer)),
		Button("start").OnClick(func() { j.start() }),
	).Spacing(10).Padding(14)
}

func main() {
	Run(&Jobs{status: "idle"}, Title("tasks"))
}
```

スクリプトは、次の手順に進む前にタスクが終わるのを待ちます。
だから `click:start` のあとの `dump` は、どちらの実行でも結果を出力します。
フレームワークのライブラリ（ダイアログや問い合わせ）は、タスクの goroutine からでも、アプリが自分で始めた goroutine からでも呼べます。
一つの呼び出しは、終わるまで同じスレッドで動きます。
アプリの側で気にすることは何もありません。


## 書いているあいだ

`gomamochi run` はアプリのファイルを監視しています。
保存すると、その変更がウィンドウに反映されます。
ファイルは新しいインタプリタで読み直され、ウィンドウが持っているアプリが、自分の値を新しいアプリに引き継ぎます。
名前と型が同じフィールドは、それまでの値をそのまま保ちます。
新しいファイルで足したフィールドは `main` が与えた値から始まり（与えなければゼロ値です）、型が変わったフィールドは値を引き継ぎません。
ファイルが宣言しているタイマーとショートカットは、新しいアプリに結び直されます。
コンパイルできないファイルを保存しても、ウィンドウは直前の表示のままで、端末にそのことが出ます。
次にコンパイルできるファイルを保存すれば、それが効きます。

保存するとファイル全体が読み直され、`main` も走り直します。
ただし、`main` がアプリに渡す初期値で、ウィンドウの持っている値が置き換わることはありません。
値は引き継がれます。
それがこの仕組みの狙いです。

