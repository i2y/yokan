# 制御フローとデータ

[ツアー](tour.md)の続きです。ハンドラに書けること、CPython と同じ意味の算術、リスト、チャート、辞書を見ます。

## ハンドラと制御フロー

ハンドラは三つの形で渡せます。
lambda（複数の操作はタプル `lambda: (a.set(x), b.set(y))`）、モジュールレベルの def、そしてストアのメソッド参照（`on_click=Cart.clear`）です。

デコレータもコンパイルできます。
デコレートはインポート時に起きるもので、コンパイル済みのアプリはモジュールを実行しません。
そこでラッパは、デコレートされたハンドラの本体に畳み込まれます。

```python
def announced(f):
    def wrapper():
        status.set("working")
        f()
        status.set("done")

    return wrapper

@announced
def save():
    fs.write_text(path, body())
```

デコレータは引数ひとつの def で、その引数をそのまま返すか、その引数を一度だけ呼ぶラッパを定義して返します。
自分が引数を取るデコレータ、関数を二度呼ぶラッパ、値として使うラッパは名指しで断られます。

def ハンドラの中身は、if や for などの制御フローごとコンパイルされます。

```python
def double(v: int) -> int:          # 純粋ヘルパはネイティブの関数になる
    return v * 2

def tally():
    total.set(0)
    for i in range(1, 6):
        if i == 3:
            continue
        total.set(total() + double(i))
```

`if` / `elif` / `else`、`while`（`while True:` も含みます）、`for`（`range()`、リストの状態、リストのフィールド、リスト型の引数）、`break` / `continue`、ローカル変数（Python と同じく再代入可）が使えます。
`log("…")` はどちらの実行でも stderr に一行書き、`assert` と `raise` は Python の例外と同じようにその文を終わらせます（アプリは動き続けます）。
条件には bool をそのまま書け（`if on:`）、比較の連鎖（`0 < n < 10`、中央は一度だけ読みます）も、`:=` での束縛も使えます。
条件式（`a if c else b`）はハンドラの中で、int、float、str、bool について書けます。
純粋ヘルパ（引数と返り値を注釈し、`return 式` で終わる関数）はハンドラからもビューのテキストからも呼べます。
分岐の途中で `return` してよく、自分自身を呼べ、`list[...]` の引数と既定引数を取り、Value クラスやリストを返せます。

if と else の**両方**で代入したローカルは、Python と同じように分岐の後でも読めます。

```python
def judge():
    n = score()
    if n > 20:
        verdict = "high"
    else:
        verdict = "low"
    grade.set(verdict)      # 分岐の後で読める
```

片方の分岐でしか代入していないローカルを後で読むことは断られます（実行されなかったときに Python なら NameError になる形なので）。

Optional の絞り込みは walrus で書きます。

```python
if (v := sel()) is not None:
    text(f"picked {v}")      # v はこの分岐の中でだけ束縛される
else:
    text("(none)")
```

## 算術

Python の算術演算子はそのまま使えます。
`+`、`-`、`*` に加えて、`/`（結果は常に float）、`//`（負の無限大方向への切り捨て）、`%`（結果は除数の符号）、`**` も、**Python の結果そのまま**でコンパイルされます。

```python
q.set(1 / 3)          # 0.3333333333333333
d.set(-7 // 2)        # -4
r.set(7 % -2)         # -1
p.set(2 ** 10)        # 1024
```

ゼロ除算やオーバーフローが起きると、その文だけが中断されます（開発中は Python の例外として見え、リリース版でも同じ文で止まります。アプリは落ちません）。
`int ** int` の指数は非負のリテラルで書きます（指数が負だと結果の型が実行時に変わってしまうため。負の指数はどちらかを float にすれば書けます）。
失敗しうる `/` `//` `%` `**` はハンドラの中で計算し、ビューには結果を渡します。

`and`、`or`、`not` は条件の中でそのまま使えます。
bool の値としても使えます（`both.set(hot() and not cold())`）。
bool 以外に対する値としての `and` / `or` は断られます（Python では結果がどちらかの**オペランドそのもの**で、真偽値とは別物のため）。

## 文字列

文字列は Python と同じように扱えます。
メソッド、長さ、添字とスライス、`in`、型変換が使えます。

```python
name.set(raw().strip().upper())
parts.set(raw().split(","))
name.set(", ".join(parts()))
first.set(raw()[0] + raw()[1:4])          # コードポイントとスライス
n.set(len(raw()) + raw().find("a"))
if "ada" in raw().lower():
    tag.set("found")
n.set(int("42") + int(2.5) + round(2.5))  # round は Python と同じ偶数丸め
```

算術と同じく、ここは二つの実行が別のコードを使う場所です。
開発中は CPython のメソッド、コンパイル後は同じ答えを返すように書いた Rust の双子で、失敗の仕方まで同じです（`int("x")` はどちらでもその文を中断します）。
その二つを突き合わせるのがゲートです。

書式指定も Python のもので、ビューでもハンドラでも同じように書けます。

```python
text(f"{total():,}")            # 1,234,567
text(f"{ratio():.1%}")          # 12.5%
text(f"{name():>10}")           # 10 桁で右寄せ
text(f"{value():.2e}")          # 1.50e+00
```

## リスト、チャート、仮想化リスト

リストへの追加は「連結して置き直す」形で書きます。
リリース版では 1 要素の追記にコンパイルされるので、コピーのコストはありません。

```python
items.set(items() + [x])     # 追記
items.set([])                # クリア
len(items())                 # 件数
```

Python のリスト操作はハンドラの中でそのまま使えます。
`in`、スライス、`sorted` / `reversed` / `min` / `max` / `sum`、内包表記、`enumerate` と `zip`、step 付きの `range`、二つのリストの連結です。
ローカルのリストは注釈で要素の型を書きます（コンパイル側がそれを読みます）。

```python
out: list[str] = []
for i, s in enumerate(items()):
    if s != "":
        out = out + [f"{i}: {s}"]
items.set(sorted(out))
best.set(max(scores()))
```

添字は Python と同じ意味で読めます。
負の添字は後ろから数え、範囲外はその文をどちらの実行でも中断します。

```python
first.set(names()[0])        # 状態を読んで添字を引く
tail.set(names()[-1])        # 最後の要素（短すぎればその文が中断）
for i in range(len(Cart.items)):
    Cart.items[i] = "-"      # ストアの中なら `self.xs[i]` も同じ
```

チャートは float か int のリストを描きます。

```python
values: State[list[float]] = State([])
line_chart(values(), height=120.0)
bar_chart(Metrics.svc_reqs, labels=Metrics.svc_names, height=100.0)
bar_chart(Books.profit, labels=Books.months, axis=True)          # 負の月は 0 の線の下に垂れる
line_chart(series=Traffic.lines, colors=["accent", "#f38ba8"], axis=True)
```

範囲はデータ全体にまたがり、常に 0 を含むので、負の値は 0 の線の下に垂れます。`min=` / `max=` を与えれば範囲を固定できます。
`axis=True` で目盛りのラベルとグリッド線が付きます。
`series=` は `list[list[float]]` のフィールドを取り、線やバーの組を複数描きます。`colors=` は系列ごとの色、`color=` は単一系列の色です（`demo/charts.py`）。
`progress(value)` はトラックを埋めます。`width=` / `height=` が大きさ、`label=` が見出し、`indeterminate=True` は長さの分からない作業のために、値の代わりに区画を往復させます。

行数の多いリストは `list_view` に渡します。
**仮想化**されていて、行を作る関数 `row(i)` は見えている範囲についてだけ呼ばれます（10 万行でも十数回）。

```python
def row(i):
    return text(items()[i])

list_view(len(items()), row, item_height=22.0, height=200.0)
list_view(len(items()), row, item_height=22.0, grow=1.0)   # 親の残り高さを埋める
```

表は、ヘッダーと列トラックを持つ `list_view` です。
`table(columns, count, row)` は見えている行についてだけ `row(i)` を呼び、行を作る関数はセルを列ごとに一つずつ並べた `row` を返します。`widths=` はトラックの比率です。
`selected=` はその行を塗り、`on_select` はクリックされた行のインデックスを受け取ります。`sort=` / `descending=` はヘッダーの矢印を描き、`on_sort` はクリックされた列のインデックスを受け取ります。並べ替えはアプリ側が自分のリストに対して行います。
スクリプトでは `select:<先頭セル>` で行を選び、`click:<列名>` でソートします（`demo/roster.py`）。

```python
def cells(i: int):
    return row(text(Roster.names[i]), text(f"{Roster.scores[i]}"))

table(["member", "score"], len(Roster.names), cells, widths=[2.0, 1.0],
      selected=Roster.sel, on_select=Roster.pick,
      sort=Roster.sort_col, descending=Roster.desc, on_sort=Roster.sort_by, grow=1.0)
```

行番号は int としてそのまま使えます。
テキストの中でも、条件でも、その行のハンドラの中でも読めます。

```python
def line(i):
    with row(spacing=6):
        text(f"{i + 1}. {items()[i]}")
        if i == Sel.idx:
            text("*")
        button("delete", on_click=lambda: Sel.drop(i))

list_view(len(items()), line, item_height=24.0, height=200.0)
```

## 辞書

読みは `.get`、書きはキー単位、数えるのは `len`、回すのは Python の辞書と同じ形です。
キーには str なら何でも書けます（リテラル、状態の読み、ループ変数）。

```python
prices["cherry"] = 200                 # キー単位の書き込み
picked.set(prices().get("apple", -1))  # 読み: 無いときは default
if "cherry" in prices(): ...           # 所属
len(prices())                          # 件数

def scan():
    for k in prices():                 # 挿入順、Python が回るのと同じ順
        last.set(k)
    for v in prices().values():        # 同じ順
        total.set(total() + v)
    for k in sorted(prices()):         # キー順で回りたいとき
        first.set(k)
```

コンパイル後の辞書はキーを入れた順を覚えているので、回すと Python と同じ順に並びます。
素の `d[k]` 読みは断られます。
無いキーで Python は KeyError を投げるので、無いときにどうするかを言う `.get(key, default)` が読みの形です。
`.items()` も断られます。
二つの名前を一度に束ねる形にはまだコンパイル後の姿がないので、キーを回してループの中で値を読みます。

