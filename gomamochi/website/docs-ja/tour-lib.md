<!-- Written by website/tools/tourpages from the tour. Edit the tour. -->
# Go とデータと仕事

Go 自身のライブラリ、フレームワークのライブラリ、タイマー、ウィンドウの外でする処理、そして保存に追いつくウィンドウ。

## Go 自身の標準ライブラリ

`fmt`、`strings`、`strconv`、`sort`、`math`、`time`、`encoding/json`、`encoding/csv`、`regexp`、`os`、`net/http` は言語自身のもので、両方の実行が同じコンパイル済みのパッケージを呼びます。
インタプリタはそれらを実装し直しません。
コマンドに組み込まれたものを呼ぶので、`fmt.Sprintf("%.2f", x)` が返すのは両方の実行で同じバイト列で、ゲートがライブラリの二つの実装を比べることはありません。

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

インタプリタが知っているライブラリは Go 1.22 のもので、その版のパッケージと関数です。
言語についてはそれより手前で、`min` と `max`（1.21）、数の上を回る `range`（1.22）、関数の上を回る `range`（1.23）はなく、1.23 以降にライブラリへ足されたものもありません。
最初の三つは、`check` が名前を挙げてそう言います。
ファイルは `os`、ネットワークは `net/http`、大きな数は `math/big` です。
デモはファイルを読み、自分自身にページを配ってそれを取得し、CSV を読みますが、標準ライブラリのほかには何も使いません。


## フレームワークの標準ライブラリ

エンジンが仲立ちするものは、パッケージから来ます。
そこでは一つの実装が、同じ C API を通して両方の実行に答えます。
スクリプトで走らせるときは、どちらの実行でもダイアログには `file:<path>` が答え、音は鳴りません。

| 関数 | 働き |
|---|---|
| `SqliteExec(db, sql, params...)` | 文を実行し、変わった行数を答える |
| `SqliteQueryText`, `SqliteQueryInt`, `SqliteQueryRows` | 問い合わせの最初の列、最初のセル、行全体 |
| `SqliteQueryTextOr`, `SqliteQueryIntOr`, `SqliteQueryRowsOr` | 同じものだが、問い合わせが失敗しても止まらず、何も答えない |
| `ClipboardSetText(s)`, `ClipboardGetText()` | システムのクリップボード |
| `OpenDialog(title)`, `SaveDialog(name)` | プラットフォーム自身のパネル。人を待つので、`Task` の中で呼ぶ |
| `AudioPlay(path, volume)`, `AudioStop()` | WAV ファイルを鳴らして、あとは放っておく |
| `NotifySend(title, body)` | 通知 |

文には `?` を書き、値はそのあとに渡します。
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

表ができる前に走るかもしれない問い合わせ（帳簿の最初の読み込みがそうです）は、何も答えない `…Or` の形で書きます。
素の形はアプリを止めます。
ライブラリ自身がそうするのと同じです。


## タイマーと、ウィンドウの外でする処理

`Every(seconds, tick)` は、決まった間隔で `tick` を呼んでもらう宣言で、`Run` より前に書きます。
二つの実行は一つの時計で刻みます。
その時計を進めるのは、ウィンドウでは 1 フレーム、スクリプトでは `advance:<ms>` なので、同じ数のティックが両方に届きます。
ゲームが動くのも、キーボードを読むのも、ティックの中です。

`Task(work, done)` は `work` を自分の goroutine で走らせ、終わったら、その答えを渡して `done` をウィンドウのスレッドで呼びます。
`work` の中からアプリのフィールドや画面に触ることはありません。
決まりはそれだけで、答えがどこかに書き込まれるのではなく引数として返ってくるのも、そのためです。

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

スクリプトは次の手順に進む前に処理を終わらせるので、`click:start` のあとの `dump` は、両方の実行で答えを表示します。
処理は、自分の goroutine からでも、アプリが自分で始めたどの goroutine からでも、フレームワークのライブラリ（ダイアログや問い合わせ）を呼べます。
呼び出しはそれぞれ、自分が続くあいだ一つのスレッドにとどまり、アプリの側で知っておくことは何もありません。


## 書いているあいだ

`gomamochi run` はアプリのファイルを監視しています。
保存すると、その変更がウィンドウに反映されます。
ファイルは新しいインタプリタで読み直され、ウィンドウが持っているアプリは、自分の値を新しいアプリに渡します。
名前と型が同じフィールドは持っていた値を保ち、新しいファイルが足したフィールドはゼロ値から始まり、型が変わったフィールドは最初からやり直します。
ファイルが宣言するタイマーとショートカットは、新しいアプリに結び直されます。
コンパイルできないファイルを保存しても、ウィンドウは直前の表示のままで、端末にそのことが出ます。
次にコンパイルできるファイルを保存すれば、それが効きます。

保存はファイル全体の読み直しで、`main` も走り直します。
ただし `main` がアプリに与える初期値は、ウィンドウが持っている値を置き換えません。
値は引き継がれるからです。
それがこの仕組みの狙いです。

