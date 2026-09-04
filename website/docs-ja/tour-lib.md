# ライブラリと crate

[ツアー](tour.md)の続きです。エラー処理、標準ライブラリ、自分の Rust crate、CPython エスケープを見ます。

## エラー処理

迷ったら、この順で選びます。

1. **`*_or` を使う**。失敗したら既定値が返る読み方で、理由が要らない場面はこれで済みます。
   `fs.read_text_or(p, "")`、`http.get_text_or(url, "")`、`sqlite.query_int_or(p, sql, 0)`。
2. **try/except を使う**。失敗の理由が要るときの形で、Python の書き方がそのまま使えます。
   本体に複数の文、例外の種類ごとの except 節、タプル指定（`except (ValueError, KeyError) as e:`）、`else`、`finally`。
   `@py` のエスケープ関数が投げた例外もここで捕まえられ、`e` のメッセージも Python が出すものそのままです。
3. **何もしない**。捕まえなかった失敗は、その文を中断してアプリは生き続けます。
   クラッシュはしません。

```python
try:
    body.set(http.get_text(url))
except Exception as e:
    status.set(f"offline: {e}")
```

## 標準ライブラリ

標準ライブラリは二つに分かれます。
分かれ目は、名前がどこから来たかです。
どちらの半分も、リリースバイナリに Python を持ち込みません。
どのモジュールが Python のどこまで届いているかは、関数の単位で[対応状況のページ](support.md)にあります（手で書かずに生成しています）。

**Python 自身のモジュール**は、Python と同じ書き方で使います（`import math`、`import random`、`import statistics`、`import json`、`import datetime`、`import time`、`import re`、`import string`、`import textwrap`、`import bisect`、`import heapq`、`import collections`、`import itertools`）。
開発中はアプリが CPython のモジュールを import し、CPython がそれを動かします。
リリースバイナリは、CPython の意味に合わせて書いた双子を呼びます。
返す値も、失敗の仕方も、エラーの文言まで同じです。
`math.sqrt(-1)` は Python が投げるところで投げ、`statistics.mean([0.1, 0.2, 0.3])` は素朴な和が返す `0.20000000000000004` ではなく厳密な `0.2` を返し、`random.seed(1)` は両方の実行で同じメルセンヌツイスタの列を始め、`json.dumps` は要素の間の `", "` から `\uXXXX` のエスケープまで CPython と同じ文字列を書き、`date` は `timedelta` を足し、別の `date` を引き、Python と同じ書式で自分を描きます。
正規表現は、アプリを翻訳する時点で CPython 自身がコンパイルします。出荷したバイナリは Python が走らせるはずだった配列をそのまま走らせるので、後方追跡も群もフラグも CPython のもので、その方言ではありません。

```python
import json, math, random, re, statistics
from datetime import date, timedelta


def measure():
    hyp.set(math.sqrt(3.0 * 3.0 + 4.0 * 4.0))     # 5.0
    spread.set(statistics.stdev([1.5, 2.5, 4.75]))
    random.seed(42)
    roll.set(random.randint(1, 6))
    doc.set(json.dumps({"name": "momo", "tags": ["a", "b"]}))
    due.set(date(2026, 1, 1) + timedelta(weeks=6))  # 2026-02-12、木曜
    mail.set(re.findall(r"\w+@[\w.]+", line())[0])   # Match には形がありません


def view():
    text(f"circumference: {math.tau * r():.3f}")   # 純粋なのでビューからも呼べる
```

`math` と `statistics` は純粋なのでビューから呼べます。
`random` は生成器の状態を進めるので、他と同じくハンドラで呼びます。
種を撒いていない生成器が繰り返せないのは Python と同じです。
種を撒けば、gate が両方の実行を一つの列に縛れます。

**Yokan 自身のモジュール**が受け持つのは、ファイル、データベース、ネットワーク、クリップボード、通知です。
`from yokan import fs, sqlite, http, jsondoc, clock, strings, clipboard, notify` と書いて読み込みます。
同じことは Python でもできます（`pathlib`、`sqlite3`、`urllib`）。
違うのは、実装がいくつあるかです。
`math` や `re` は、開発中は CPython のモジュールが動き、リリース後は Rust の双子が動きます。
こちらはどちらの実行も Rust の同じ関数を呼ぶので、二つの実行が違う答えを返しようがありません。
これらに Python のモジュール名は使いません。
Python の名前が付いていることは、CPython の答えに合わせるという約束だからです。
JSON 文書をドットパスで読むのは `json` ではなく `jsondoc`、機械のタイムゾーンを読むのは `time` ではなく `clock` です。
呼ぶのはハンドラからです（ビューは純粋なまま）。

- **fs**：`read_text` / `write_text` / `append_text` / `exists` / `read_text_or` / `list_dir`（ディレクトリの中の名前を並べ替えて返す）/ `make_dir` / `remove` / `app_dir(name)`（このアプリが自分のファイルを置いてよいディレクトリ。無ければ作って返す）
  それと、プラットフォーム自身のパネルである `open_dialog(title)` と `save_dialog(name)`。返るのはパスで、取り消されたときは `""` です。ダイアログは人を待つので `task(...)` の中で呼びます。検証スクリプトは `file:<path>` で答えます。
- **sqlite**：`exec` / `query_text` / `query_int` / `query_rows` / `query_int_or` / `query_text_or` / `query_rows_or`（SQLite 同梱。`query_text` は各行の 0 列目、`query_rows` は全列を返す。集計は COALESCE で包み、ORDER BY で順序を固定する）
- **http**：`get_text(url)` / `get_text_or` / `get_text_with(url, headers)` / `post_text(url, body)` / `post_text_or` / `status(url)`（同期。`get_text` は第二引数にミリ秒の締め切り、`post_text` は第三引数に content type を取る）
- **jsondoc**：`get_text` / `get_int` / `get_float` / `get_bool` / `length` / `has` — JSON 文書を `"items.0.title"` のようなドットパスで読みます。Python の `json` にこの動詞はありません。書き出しは Python の `json.dumps` です
- **clock**：`format_ms(ms, "%Y-%m-%d")`（UTC。検証スクリプトでは固定の ms を渡す）、`format_local_ms(ms, fmt)`（この機械のタイムゾーン。両方の実行が同じタイムゾーンデータベースを読む）、`local_offset_minutes(ms)`。この機械のタイムゾーンは、Python の `time` からは struct 越しにしか触れません。時計そのものを読むのは Python の `time`、暦の計算は Python の `datetime` です
- **strings**：`to_int(s, default)` / `to_float(s, default)`（壊れた入力は default になる数値パース）
- **clipboard**：`set_text(s)` / `get_text()` — システムのクリップボード。ウィンドウでは他のアプリケーションとやり取りし、ヘッドレス実行では自分の中に閉じるので、コピーと貼り付けも他の操作と同じように検証できる
- **notify**：`send(title, body)` — OS 通知。`.app` バンドル（`--app`）として動かすと通知センターに届き、素の開発実行とヘッドレス実行では静かに捨てられる

Python 側がどこまで届くかを、モジュールごとに挙げます。

- **math** — 六つを除いて全部です。除いた六つはそれぞれ理由を名指しして断ります。`prod` と `sumprod` はリストの中身によって int か float かが変わり、`gamma`、`lgamma`、`erf`、`erfc` はプラットフォームではなく CPython 自身が計算しているからです。
- **random** — `seed`、`random`、`randint`、`randrange`、`getrandbits`、`uniform`、`gauss`、`choice`、`sample`。
- **statistics** — `mean`、`fmean`、`median`、`mode`、`variance`、`pvariance`、`stdev`、`pstdev`。受けるのは `list[float]` だけです。int のリストは断ります。CPython は `mean([1, 2, 3])` に int を、`mean([1, 2, 4])` に float を返すので、型が一つに決まらないからです。
- **json** — `dumps`。既定値のままで、キーワード引数は取りません。
- **time** — `time`、`time_ns`、`monotonic`、`monotonic_ns`、`perf_counter`、`perf_counter_ns`、`sleep`。
- **re** — `findall`、`sub`、`split`、`escape` と、判定としての `re.search(p, s) is not None`（`match` と `fullmatch` も同じ）。パターンはリテラルです。アプリを翻訳する時点でコンパイルするからです。
- **datetime** — `date`、`datetime`、`timedelta` の三つで、いずれも naive です。構築、`today` / `now` / `fromisoformat` / `fromtimestamp` / `fromordinal` / `combine`、各部分（`.year`、`.hour`、`.days` など）、`isoformat`、`strftime`、`weekday`、`toordinal`、`timestamp`、`total_seconds`、算術と比較。穴に置いた値は `str()` と同じ形で描かれます。
- **collections** — `Counter`（str のリストを数えます）。結果は初めて現れた順に並ぶ辞書で、辞書が答えるものすべてに加えて `.most_common()` と `.total()` を持ちます。`State` に入れると辞書として読み戻るので、順位は入れる前に取り出します。
- **itertools** — `chain`、`pairwise`、`accumulate`、`combinations`、`permutations`、`product`。どれも Python ではイテレータを返すので、ここでは `for` で回すものになります。
- **string / textwrap / bisect / heapq** — 九つの定数、`dedent` と `indent`、`bisect_left` と `bisect_right`、`nsmallest` と `nlargest`。

```python
c = Counter(votes())                       # {"ivy": 3, "momo": 2, "ada": 1}
for name, n in c.most_common(2):           # 個数の多い順、同数なら現れた順
    board.set(board() + f"{name}:{n} ")

for a, b in itertools.pairwise(readings()):
    steps.set(steps() + [b - a])
```

sqlite の呼び出しは、どれも最後にバインドする値のリストを取れます。

```python
sqlite.exec(DB, "INSERT INTO expenses VALUES (?, ?, ?)", [item, str(yen), cat])
sqlite.query_int_or(DB, "SELECT COALESCE(SUM(amount),0) FROM expenses WHERE cat=?", 0, ["food"])
```

値の位置に `?` を書き、値は文の外に並べて渡します。
こう書けば `item` の中のアポストロフィはアポストロフィのままで、利用者が打った文字列が SQL になることはありません。
値はテキストとしてバインドされ、列の affinity が変換します。
INTEGER の列には数値が入ります。

行はまるごと `list[str]` として返るので、結果は `list[list[str]]` です。

```python
@store
class Ledger:
    raw: list[list[str]] = []
    rows: list[str] = []

    def load(self) -> None:
        self.raw = sqlite.query_rows_or(DB, "SELECT name, amount, cat FROM expenses ORDER BY rowid")
        self.rows = []
        for r in self.raw:
            self.rows = self.rows + [f"{r[0]}  ¥{r[1]}  ({r[2]})"]
```

表示する一行は、SQL で組み立てるのではなく Python 側で書きます。

検証を安定させるこつは、結果を毎回同じにすることです。
時刻は固定値を渡し、乱数は種を撒く。
そうしておけば、検証スクリプトは何度でも同じ結果を再生します。

自分の Rust crate を足すこともできます。
それが次の節です。

## Rust crate を呼ぶ

Rust の crate を宣言して、アプリから呼べます。
crates.io の version 指定でも、手元の path 指定でも構いません。
追加は 1 コマンドです。

```console
$ yokan add app.py deunicode 1                    # crates.io から
$ yokan add app.py hexfmt --path native/hexfmt    # 手元の crate
```

宣言の置き場はアプリの流儀に合わせて二つあります。
スクリプト型なら PEP 723 ブロックの `[tool.yokan.crates]`、プロジェクト型なら pyproject.toml の同じテーブルです（`yokan add` がどちらの家も見つけて書き込みます）。

```python
# /// script
# requires-python = ">=3.14"
#
# [tool.yokan.crates]
# hexfmt = { path = "native/hexfmt" }
# ///
from yokan import crates

# ハンドラの中で
self.encoded = crates.hexfmt.encode("yokan")
self.total = crates.hexfmt.add(40, 2)
self.mean = crates.hexfmt.avg(self.samples)
```

crate 側は普通の Rust で、pyo3 も yokan の型も要りません。

```rust
pub fn encode(s: &str) -> String { … }
pub fn add(a: i64, b: i64) -> i64 { … }
pub fn avg(xs: Vec<f64>) -> f64 { … }
```

`yokan gate` と `yokan build` が、どちらの実行からも呼べるように crate を用意します。
ゲートを通さず `uv run` だけで動かしたいときは、先に一度 `yokan sync app.py` を実行します。

この機能はネイティブビルドと同じ前提です（リポジトリの clone と Rust）。
関数名は crate のドキュメント通りの snake_case で呼びます。
境界を越えられるのは、Int、Float、Bool、String、その List と Optional（None ごと）、str キーの辞書（`HashMap<String, …>`）、構造体（入れ子も）と enum、そして Result を返す関数です（`Result<Vec<…>>` のような複合型も可）。
crate から返る辞書はキー順に並んで届きます。どちらの実行でも同じ順です。
Result は try/except で受け、`f"{e}"` の文言まで両実行で一致します。
構造体と enum は、アプリ側に同名の**双子**を宣言すると往復します。
特別な印は要りません。同じ形で宣言するだけです。
入れ子の構造体は、内側の双子を先に宣言して、外側のフィールドにその名前を書きます。

```python
@value
class Span:          # crate の struct Span の双子
    lo: int
    hi: int


class Grade(Enum):   # crate の enum Grade の双子
    Fine = 1
    Odd = 2

moved = crates.hexfmt.shift(Span(3, 8), 10)
self.verdict = crates.hexfmt.describe(crates.hexfmt.judge(7))
```

Rust 側が `u32` などの幅付きフィールドを持つ構造体も、そのまま越えます（読みは広がり、書きは幅に合わせて戻ります）。入れ子のフィールドも同じ規則です。
越えられない型を呼ぶと、何がなぜだめかを名指しするエラーになります。
デモは `demo/rustcrate.py`（path と version の同居、Optional・Result・構造体・enum・辞書まで）と `demo/proj/`（pyproject 綴り）です。

## CPython エスケープ

ここまでの範囲から外れる Python が要るときは、関数に `@py` を付けます（`from yokan import py`）。
その関数は**本物の Python のまま**残ります。
開発中はそのまま、リリース後は同梱または実行環境の CPython で実行されます（自己完結にするなら後述の `--bundle` / `--onefile`）。

```python
@py
def slug(t: str) -> str:
    import re                  # import はエスケープの中に書く
    return re.sub(r"[^a-z0-9]+", "-", t.lower()).strip("-")
```

引数と返り値は全部注釈します（int、float、str、bool、それらの `list[...]` と `dict[str, ...]`、Value クラス、`T | None`）。
numpy のようなコンパイル済み拡張もエスケープの中で使えます。

## 重い処理とタイマーとキー

ハンドラをブロックしてはいけません（ウィンドウが固まります）。
`task` がワーカースレッドで仕事をして、終わったら UI スレッドで続きを実行します。

```python
def start():
    busy.set(True)
    task(fetch_data, on_done=lambda v: (busy.set(False), data.set(v)))
```

`on_error=` は開発実行だけの形です。
標準ライブラリ呼び出しの失敗は、その呼び出しを `try` / `except` で囲んで受けます。

渡す関数は UI 要素を作らず、値を返すだけにします。
`task` はそのハンドラの最後の文にします（Python では task の後の文が仕事の完了より先に走るためです）。
ヘッドレス実行はタスクの完了を待ってから次のステップに進むので、タスクを含む流れもテストできます。
どちらの実行も同じことをします。
開発実行では Python のスレッドが、コンパイル済みの実行では中の標準ライブラリ呼び出しの `await` が、その仕事を UI スレッドの外に出します。
task の中の純粋な計算は書いた場所で走ります。
外に出るのは `fs`、`sqlite`、`http`、`time.sleep_ms` の呼び出しです。

`every(seconds, cb)` は秒間隔のタイマーで、モジュールレベル（または `__main__` ガードの中）に書いて、アプリと一緒に始まります。

```python
def tick():
    n.set(n() + 1)

every(1.0, tick)
```

これは後から呼ぶものではなく宣言です。
どちらの実行もアプリの開始時にタイマーを始め、同じ時計で発火します（ウィンドウならフレーム、ヘッドレスなら `advance:<ms>`）。
そのため一分ぶんのティックもゲートで確かめられます。

キーも同じように宣言します。
`shortcut(chord, handler)` はコードをひとつ束ね、`on_key(handler)` はすべてのキーをコードの形で受け取ります。

```python
def save():
    fs.write_text(path, body())

shortcut("cmd+s", save)
on_key(lambda k: last.set(k))
```

コードの綴りはプラットフォームの綴りに合わせます（`cmd+s`、`shift-tab`、`ctrl+alt+k`）。
`-` で区切っても同じものとして読みます。
テキストフィールドにキャレットがある間、修飾のないキーはそのフィールドへの入力のままで、cmd か ctrl を伴うコードだけがアプリに届きます。
ヘッドレスのスクリプトは `key:cmd+s` で押せるので、ショートカットもクリックと同じく検証される操作になります。

`menu_item(menu, name, handler)` は、同じハンドラをアプリケーションのメニューバーに置きます。

```python
menu_item("File", "Save", save)
menu_item("File", "Clear", clear)
```

宣言した順がメニューの順で、ウィンドウはこのバーをプラットフォームに渡します。
スクリプトからは `menu:Save` のように名前で選びます。

ウィンドウに落とされたファイルも同じ形で宣言します。
`on_file_drop(handler)` のハンドラがパスを受け取り、スクリプトは `drop:<path>` で落とします。

