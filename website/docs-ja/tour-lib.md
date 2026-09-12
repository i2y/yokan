# ライブラリと crate

[ツアー](tour.md)の続きです。
エラー処理、標準ライブラリ、自分の Rust crate、CPython エスケープを見ます。

## エラー処理

迷ったら、この順で選びます。

1. **`*_or` を使う**。
   失敗したら既定値が返る読み方です。
   失敗の理由が要らない場面は、これで済みます。
   `fs.read_text_or(p, "")`、`http.get_text_or(url, "")`、`sqlite.query_int_or(p, sql, 0)`。
2. **try/except を使う**。
   失敗の理由が要るときはこの形で、Python の書き方がそのまま使えます。
   書けるのは、本体の複数の文、例外の種類ごとの except 節、タプル指定（`except (ValueError, KeyError) as e:`）、`else`、`finally` です。
   失敗しうる標準ライブラリの呼び出し（書き込み、問い合わせ、JSON の経路による読み取り、時刻の整形）は、どれもここで捕まえられます。
   `@py` のエスケープ関数が投げた例外も、ここで捕まえられます。
   `e` のメッセージも、Python が出すものそのままです。
3. **何もしない**。
   捕まえなかった失敗はその文を中断させるだけで、アプリはクラッシュせずに動き続けます。

```python
try:
    body.set(http.get_text(url))
except Exception as e:
    status.set(f"offline: {e}")
```

## 標準ライブラリ

標準ライブラリは二つに分かれます。
分かれ目は、名前がどこから来たかです。
どちらも、リリースバイナリに Python を持ち込みません。
どのモジュールが Python のどこまでを実装しているかは、関数の単位で[対応状況のページ](support.md)にあります（手で書かずに生成しています）。

**Python 自身のモジュール**は、Python と同じ書き方で使います（`import math`、`import random`、`import statistics`、`import json`、`import datetime`、`import time`、`import re`、`import string`、`import textwrap`、`import bisect`、`import heapq`、`import collections`、`import itertools`）。
開発中はアプリが CPython のモジュールを import し、CPython がそれを動かします。
リリースバイナリは、CPython の意味に合わせて書いた双子を呼びます。
返す値も、失敗の仕方も、エラーの文言まで同じです。
`math.sqrt(-1)` は、Python が投げるところで投げます。
`statistics.mean([0.1, 0.2, 0.3])` は、素朴な和が返す `0.20000000000000004` ではなく厳密な `0.2` を返します。
`random.seed(1)` のあとは、両方の実行が同じメルセンヌツイスタの列をたどります。
`json.dumps` は、要素の間の `", "` から `\uXXXX` のエスケープまで CPython と同じ文字列を書きます。
`date` は `timedelta` を足せ、別の `date` を引け、文字列にすれば Python と同じ書式になります。
正規表現は、アプリを翻訳する時点で CPython 自身がコンパイルします。
リリースしたバイナリは、Python が走らせるはずだった配列をそのまま走らせます。
後方追跡も群もフラグも CPython のもので、方言が真似たものではありません。

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
種を与えていない生成器が繰り返せないのは Python と同じです。
種を与えれば、ゲートは両方の実行を同じ列に固定できます。

**Yokan 自身のモジュール**が受け持つのは、ファイル、データベース、ネットワーク、クリップボード、通知です。
`from yokan import fs, sqlite, http, jsondoc, clock, strings, clipboard, notify, audio, keys` と書いて読み込みます。
同じことは Python でもできます（`pathlib`、`sqlite3`、`urllib`）。
違うのは、実装がいくつあるかです。
`math` や `re` は、開発中は CPython のモジュールが動き、リリース後は Rust の双子が動きます。
こちらはどちらの実行も Rust の同じ関数を呼ぶので、二つの実行が違う答えを返しようがありません。
これらに Python のモジュール名は使いません。
Python の名前を名乗ることが、CPython の答えに合わせるという約束だからです。
JSON 文書をドットパスで読むのは `json` ではなく `jsondoc`、機械のタイムゾーンを読むのは `time` ではなく `clock` です。
呼ぶのはハンドラからです（ビューは純粋なまま）。

- **fs**：`read_text` / `write_text` / `append_text` / `exists` / `read_text_or` / `list_dir`（ディレクトリの中の名前を並べ替えて返す）/ `make_dir` / `remove` / `app_dir(name)`（このアプリが自分のファイルを置いてよいディレクトリ。無ければ作って返す）/ `size`（ファイルの長さ。バイト数）/ `modified_ms`（最後に書かれた時刻。`clock.format_ms` が読むミリ秒）/ `is_dir` / `read_text_from(path, offset)`（バイト位置から末尾までの文字列。一度末尾まで読んだファイルの続きで、伸びていくログを先頭から読み直さずに追いかける読み方。末尾かその先を指せば `""` を返す）/ `read_bytes` / `write_bytes`（ファイルをまるごと `bytes` として読み書きする。画像や書庫、ダイジェストの入力のように、文字列では運べないもののため）
  それと、プラットフォーム自身のダイアログを開く `open_dialog(title)` と `save_dialog(name)`。
  返るのはパスで、取り消されたときは `""` です。
  ダイアログは人を待つので `task(...)` の中で呼びます。
  検証スクリプトでは、`file:<path>` がこのダイアログに答えます。
- **sqlite**：`exec` / `query_text` / `query_int` / `query_rows` / `query_int_or` / `query_text_or` / `query_rows_or`（SQLite 同梱。`query_text` は各行の 0 列目、`query_rows` は全列を返す。集計は COALESCE で包み、ORDER BY で順序を固定する）
- **http**：`get_text(url)` / `get_text_or` / `get_text_with(url, headers)` / `get_bytes(url)` / `post_text(url, body)` / `post_text_or` / `status(url)`（同期。`get_text` は第二引数にミリ秒単位の制限時間、`post_text` は第三引数に content type を取る。`get_bytes` は応答を `bytes` で返すので、文字列でないものを受け取れる）
- **jsondoc**：`get_text` / `get_int` / `get_float` / `get_bool` / `length` / `has` — JSON 文書を `"items.0.title"` のようなドットパスで読みます。`get_texts(src, paths, default)` はその読みを一回の解析でまとめて行います。パスのリストを渡すと文字列のリストが返り（数値と真偽値は JSON が書く形、リストとマップはその JSON）、パスの先に何もなければ `default` になります。ログの一行を分解する読み方で、レコードが毎回すべての項目を持つとは限らないからです。
  Python の `json` に、同じ読み方はありません。
  書き出しは Python の `json.dumps` です
- **clock**：`format_ms(ms, "%Y-%m-%d")`（UTC。検証スクリプトでは固定の ms を渡す）、`format_local_ms(ms, fmt)`（この機械のタイムゾーン。両方の実行が同じタイムゾーンデータベースを読む）、`local_offset_minutes(ms)`。
  この機械のタイムゾーンは、Python の `time` からは struct 越しにしか触れません。
  時計そのものを読むのは Python の `time`、暦の計算は Python の `datetime` です
- **strings**：`to_int(s, default)` / `to_float(s, default)`（壊れた入力は default になる数値パース）
- **clipboard**：`set_text(s)` / `get_text()` — システムのクリップボード。
  ウィンドウでは他のアプリケーションとやり取りし、ヘッドレス実行ではアプリの中で完結するので、コピーと貼り付けも他の操作と同じように検証できる
- **notify**：`send(title, body)` — OS 通知。
  `.app` バンドル（`--app`）として動かすと通知センターに届き、素の開発実行とヘッドレス実行では静かに捨てられる
- **audio**：`play(path, volume=1.0)` / `stop()` — WAV を鳴らす。
  呼び出しはすぐ戻り、複数の音は重なって鳴る。
  `volume` は 0 から 1 の音量（大きすぎる音は取り返しがつかないので、秒に何度も鳴るものには小さめの値を渡す）。
  スクリプト実行は無音なので、ゲートにスピーカーのある機械は要らず、音がダンプに出ることもない。
  音の出せない機械や読めないファイルでは、アプリを止めずに何も鳴らさない。
  音のデバイスを組み込むのはこれを import したアプリだけで、バイナリの増分はおよそ 1.3 MB
- **keys**：`down(k)` / `pressed(k)` / `released(k)` — 装置としてのキーボード。
  タイマーのティックから読む（ハンドラに届くキーの組み合わせのほうは[ウィンドウ](tour-ui.md#ウィンドウ)を参照）

Python のどこまでを実装しているかを、モジュールごとに挙げます。

- **math** — 六つを除いて全部です。
  除いた六つは、それぞれ理由を挙げて断ります。
  `prod` と `sumprod` はリストの中身によって int か float かが変わり、`gamma`、`lgamma`、`erf`、`erfc` はプラットフォームではなく CPython 自身が計算しているからです。
- **random** — `seed`、`random`、`randint`、`randrange`、`getrandbits`、`uniform`、`gauss`、`choice`、`sample`。
- **statistics** — `mean`、`fmean`、`median`、`mode`、`variance`、`pvariance`、`stdev`、`pstdev`。
  受けるのは `list[float]` だけで、int のリストは断ります。
  CPython は `mean([1, 2, 3])` に int を、`mean([1, 2, 4])` に float を返すので、型が一つに決まらないからです。
- **json** — `dumps`。
  既定値のままで、キーワード引数は取りません。
- **time** — `time`、`time_ns`、`monotonic`、`monotonic_ns`、`perf_counter`、`perf_counter_ns`、`sleep`。
- **re** — `findall`、`sub`、`split`、`escape` と、判定としての `re.search(p, s) is not None`（`match` と `fullmatch` も同じ）。
  パターンはリテラルだけです。
  アプリを翻訳する時点でコンパイルするからです。
- **datetime** — `date`、`datetime`、`timedelta` の三つで、ゾーンを与えないかぎりタイムゾーンなしの値です（ゾーンは下の `zoneinfo` を見てください）。
  使えるのは、構築、`today` / `now` / `fromisoformat` / `fromtimestamp` / `fromordinal` / `combine`、各部分（`.year`、`.hour`、`.days` など）、`isoformat`、`strftime`、`weekday`、`toordinal`、`timestamp`、`total_seconds`、それに算術と比較です。
  穴に置いた値は `str()` と同じ形で描かれます。
- **collections** — `Counter`（str のリストを数えます）。
  結果は初めて現れた順に並ぶ辞書で、辞書にできることすべてに加えて `.most_common()` と `.total()` を持ちます。
  `State` に入れると辞書として読み戻るので、順位は入れる前に取り出します。
- **itertools** — `chain`、`pairwise`、`accumulate`、`combinations`、`permutations`、`product`。
  どれも Python ではイテレータを返すので、ここでは `for` で回すものになります。
- **string / textwrap / bisect / heapq** — 九つの定数、`dedent` と `indent`、`bisect_left` と `bisect_right`、`nsmallest` と `nlargest`。
- **hashlib** — `sha256`、`sha1`、`md5` の三つを `.hexdigest()` で読みます。
  Python はダイジェストをハッシュオブジェクト越しの二回の呼び出しで書きますが、途中で書き換わるオブジェクトはコンパイルする形を持たないので、方言はこの二つを一組として読みます。
- **base64** — `b64encode` と `b64decode`。
  Python のものと同じく、どちらも `bytes` を返します。
- **zoneinfo** — `ZoneInfo(key)` と、タイムゾーン付きの `datetime` についてゾーンが決めることすべてです。
  `now(tz)`、`fromtimestamp(ts, tz)`、`datetime(..., tzinfo=tz)`、`astimezone`、`utcoffset`、`dst`、`tzname`、オフセット付きの `isoformat`、`strftime` の `%z` と `%Z`、二つの値の比較と差、`+ timedelta` です。
  両方の実行がこの機械のゾーンファイルを読むので、オフセットは二つの実行が食い違える対象ではありません。

ゾーンは値ではなく型のほうに乗ります。
キーを書いた場所でそのまま読む書き方になるのはそのためで、コンパイル側は翻訳しながらキーを読みます。
値そのものはタイムゾーンなしの `datetime` と同じ整数（そのゾーンでの壁時計）なので、`.year` や `.hour` はそのまま読めますし、注釈しか手がかりのない `State` やフィールドは、これまでどおりタイムゾーンなしの値を持ちます。

```python
TOKYO = ZoneInfo("Asia/Tokyo")
NEW_YORK = ZoneInfo("America/New_York")

here = datetime(2026, 7, 14, 9, 30, tzinfo=TOKYO)
there = here.astimezone(NEW_YORK)           # 2026-07-13 20:30:00-04:00
text(f"{there.strftime('%H:%M %Z')}")       # 20:30 EDT
```

`utcoffset()` と `tzname()` は typeshed では `| None` の型を持ちます。
タイムゾーンなしの値にはどちらも無いからです。
穴に置いて描くぶんには問題になりませんが、数そのものが欲しいときは `strftime("%z")` が同じことを絞り込みなしで言います。

`bytes` はそれ自身が一つの型で、振る舞いは Python のものと同じです。
リテラルはエスケープも含めて `b"..."` で、`s.encode()` が文字列からバイト列を作り、`b.decode()` が文字列に戻します。
`len(b)` は個数、`b[i]` は数値、`b[a:b]` はバイト列、`+` は連結で、`.hex()` と `bytes.fromhex(s)` が文字列との行き来です。
`State[bytes]` と `bytes` のフィールドも持てて、どちらも `b""` から始まります。

```python
raw = phrase().encode()                    # b'yokan'
stamp.set(hashlib.sha256(raw).hexdigest()) # 61aca55e4c72…
packed.set(f"{base64.b64encode(raw)}")     # b'eW9rYW4='
fs.write_bytes(path, PNG + raw)            # リテラルと値の連結
```

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

検証を安定させるこつは、実行のたびに変わる要素をなくすことです。
時刻は固定値を渡し、乱数には種を与えます。
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
スクリプト型なら PEP 723 ブロックの `[tool.yokan.crates]`、プロジェクト型なら pyproject.toml の同じテーブルです（`yokan add` はどちらの置き場も見つけて書き込みます）。

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
crate から返る辞書はキー順に並んで届きます。
どちらの実行でも同じ順です。
Result は try/except で受け、`f"{e}"` の文言まで両実行で一致します。
構造体と enum は、アプリ側に同名の**双子**を宣言すると往復します。
特別な印は要りません。
同じ形で宣言するだけです。
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

Rust 側が `u32` などの幅付きフィールドを持つ構造体も、そのまま越えます（読みは広がり、書きは幅に合わせて戻ります）。
入れ子のフィールドも同じ規則です。
越えられない型を使う関数を呼ぶと、何がなぜだめかがエラーに出ます。
デモは `demo/rustcrate.py`（path と version の同居、Optional、Result、構造体、enum、辞書まで）と `demo/proj/`（pyproject に書く形）です。

## CPython エスケープ

ここまでの範囲から外れる Python が要るときは、関数に `@py` を付けます（`from yokan import py`）。
その関数は**本物の Python のまま**残ります。
開発中はそのまま動き、リリース後は同梱した CPython か実行環境の CPython が動かします（自己完結にするなら後述の `--bundle` / `--onefile`）。

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
`task` がワーカースレッドでタスクを走らせ、終わったら UI スレッドで続きを実行します。

```python
def start():
    busy.set(True)
    task(fetch_data, on_done=lambda v: (busy.set(False), data.set(v)))
```

`on_error=` は開発実行だけの形です。
標準ライブラリ呼び出しの失敗は、その呼び出しを `try` / `except` で囲んで受けます。

渡す関数は UI 要素を作らず、値を返すだけにします。
アプリの状態も読めません。
タスクは UI スレッドの外で走り、そこから状態には手が届かないからです。
必要な値は task の前に読んで引数で渡し、返ってきた値は `on_done` で書きます。
`task` はそのハンドラの最後の文にします（Python では task の後の文がタスクの完了より先に走るためです）。
ヘッドレス実行はタスクの完了を待ってから次のステップに進むので、タスクを含む流れもテストできます。
どちらの実行も同じことをします。
開発実行では Python のスレッドが、コンパイル済みの実行では中の呼び出しの `await` が、そのタスクを UI スレッドの外に出します。
task の中の純粋な計算は、書いた場所から動きません。
UI スレッドの外に出るのは `fs`、`sqlite`、`http`、`time.sleep` の呼び出しと、`@py` のエスケープです。
一分かかる Python を書いてもウィンドウが描き続けるのは、エスケープが外に出るからです。

走っている間の進み具合も伝えられます。
`report(fraction, note)` がそれを伝え、`on_progress` が UI スレッドで受け取ります。
呼ぶ場所はタスクの中でも、そこから呼んだ `@py` のエスケープの中でもかまいません。

```python
def moved(fraction: float, note: str):
    done.set(fraction)
    step.set(note)


def start():
    task(count_primes, on_done=counted, on_progress=moved)
```

エスケープの中では `from yokan import report` と書いて取り込みます。
報告はすべて届きます。
最後のひとつが届くのは `on_done` より先です。
task の外で呼んだときは何も起きません。
報告の中身はタスクの側の話で、機械によって変わることもあります。
届くという事実のほうは変わらないので、届いた回数はゲートで比べられます。

`every(seconds, cb)` は秒間隔のタイマーです。
モジュールレベル（または `__main__` ガードの中）に書くと、アプリと一緒に動き始めます。

```python
def tick():
    n.set(n() + 1)

every(1.0, tick)
```

これは後から呼ぶものではなく宣言です。
どちらの実行もアプリの開始時にタイマーを始め、同じ時計で発火します（ウィンドウならフレーム、ヘッドレスなら `advance:<ms>`）。
そのため一分ぶんのティックもゲートで確かめられます。

キーも同じように宣言します。
`shortcut(chord, handler)` はキーの組み合わせをひとつ結びつけ、`on_key(handler)` はすべてのキーを組み合わせの形で受け取ります。

```python
def save():
    fs.write_text(path, body())

shortcut("cmd+s", save)
on_key(lambda k: last.set(k))
```

キーの組み合わせは、プラットフォームの綴りで書きます（`cmd+s`、`shift-tab`、`ctrl+alt+k`）。
`-` で区切っても同じものとして読みます。
`cmd` はアプリ自身のショートカットを載せるキーで、独立したキーを持つのは macOS だけです。
Windows と Linux ではそれが Ctrl なので、`cmd+s` と `ctrl+s` はひとつの組み合わせを指し、スクリプトはどのプラットフォームでも `key:cmd+s` で押します。
テキストフィールドにキャレットがある間、修飾のないキーはそのフィールドへの入力のままで、cmd か ctrl を伴う組み合わせだけがアプリに届きます。
ヘッドレスのスクリプトは `key:cmd+s` で押せるので、ショートカットもクリックと同じく検証される操作になります。

キーの組み合わせは押した瞬間に一度届くだけですが、押されたままのキーはそうではありません。
それに答えるのが `keys` です。

```python
from yokan import keys

def tick():
    if keys.down("left"):
        Game.steer(-1)
    if keys.pressed("space"):
        Game.fire()

every(0.033, tick)
```

`keys.down(name)` は「いま押されている」、`keys.pressed(name)` は「前のティック以降に押された」、`keys.released(name)` はその反対です。
名前に書けるのは修飾のないキー1つ（`left`、`space`、`z`）で、修飾キー自身もそれぞれの名前（`shift`、`cmd`、`ctrl`、`alt`）で読めます。
`down("left")` は shift を一緒に押していても真です。

読むのはティックの中で、ビューの中ではありません。
ビューはフレームワークの都合で組み直されるので、そこで読んだ瞬間はアプリが選んだ瞬間ではないからです（ビューの中の時計を断るのと同じ理由です）。
`pressed` と `released` が見たものは、それを読んだティックで使い切ります。
だからキーを押しっぱなしにしても、何フレーム続こうと1回です。
スクリプトは `keydown:left` で押し、`keyup:left` で離します。
`key:<chord>` はその両方を一度に行うので、ゲームも1フレームずつゲートで比べられます。

`menu_item(menu, name, handler)` は、同じハンドラをアプリケーションのメニューバーに置きます。

```python
menu_item("File", "Save", save)
menu_item("File", "Clear", clear)
```

宣言した順がメニューの順で、ウィンドウはこのバーをプラットフォームに渡します。
スクリプトからは `menu:Save` のように名前で選びます。
三つのプラットフォームのうち、ここからメニューバーを描くのは macOS だけです。
Linux と Windows は宣言を保持したまま何も出しませんが、`menu:Save` はどれでもハンドラを呼びます。

ウィンドウに落とされたファイルも同じ形で宣言します。
`on_file_drop(handler)` のハンドラがパスを受け取り、スクリプトは `drop:<path>` で落とします。

