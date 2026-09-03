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

`from yokan import fs, sqlite, http, math, json, time, strings, random, notify` で使います。
どれも Rust で実装された同じ関数を、開発中もリリース後も呼びます。
リリースバイナリに Python は要りません。
呼ぶのはハンドラからです（ビューは純粋なまま）。

- **fs**：`read_text` / `write_text` / `append_text` / `exists` / `read_text_or` / `list_dir`（ディレクトリの中の名前を並べ替えて返す）/ `make_dir` / `remove` / `app_dir(name)`（このアプリが自分のファイルを置いてよいディレクトリ。無ければ作って返す）
- **sqlite**：`exec` / `query_text` / `query_int` / `query_rows` / `query_int_or` / `query_text_or` / `query_rows_or`（SQLite 同梱。`query_text` は各行の 0 列目、`query_rows` は全列を返す。集計は COALESCE で包み、ORDER BY で順序を固定する）
- **http**：`get_text(url)` / `get_text_or` / `get_text_with(url, headers)` / `post_text(url, body)` / `post_text_or` / `status(url)`（同期。`get_text` は第二引数にミリ秒の締め切り、`post_text` は第三引数に content type を取る）
- **math**：`sqrt` / `sin` / `cos` / `pow` / `fabs` / `floor` / `ceil` / `pi`
- **json**：`get_text` / `get_int` / `get_float` / `get_bool` / `length` / `has`（`"items.0.title"` のようなドットパスで引く）と `dumps(value)`（str、int、float、bool、そのいずれかのリスト、str をキーとする dict を書き出す。dict はキー順）
- **time**：`now_ms`、`format_ms(ms, "%Y-%m-%d")`（UTC。検証スクリプトでは固定の ms を渡す）、`format_local_ms(ms, fmt)`（この機械のタイムゾーン。両方の実行が同じタイムゾーンデータベースを読む）、`local_offset_minutes(ms)`、`sleep_ms(ms)`（呼び出し側を止めます。`task` の中ならコンパイル済みの実行は `await` します）
- **strings**：`to_int(s, default)` / `to_float(s, default)`（壊れた入力は default になる数値パース）
- **random**：`seed(n)` / `int(lo, hi)`（両端含む）/ `float()`（種を撒けば毎回同じ列）
- **notify**：`send(title, body)` — OS 通知。`.app` バンドル（`--app`）として動かすと通知センターに届き、素の開発実行とヘッドレス実行では静かに捨てられる

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

仕組みは標準ライブラリと同じ「実装ひとつ、入口ふたつ」です。
開発中の CPython 向けには pyo3 の入口が自動生成されてビルドされ、リリース向けにはバインディングが rustdoc の JSON 出力から自動導出されます。
どちらも `yokan gate` / `yokan build` が面倒を見ます。
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

ここまでの範囲の外の Python が要るときは、関数に `@py` を付けます（`from yokan import py`）。
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

## 重い処理とタイマー

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

