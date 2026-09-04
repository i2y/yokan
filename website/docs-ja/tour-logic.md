# 制御フローとデータ

[ツアー](tour.md)の続きです。ハンドラに書けること、CPython と同じ意味の算術、リスト、チャート、辞書、タプルを見ます。

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
自分が引数を取るデコレータ、関数を二度呼ぶラッパ、値として使うラッパは断られます。

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
`in`、スライス、`sorted` / `min` / `max` / `sum`、内包表記、`enumerate` と `zip`、step 付きの `range`、二つのリストの連結です。
ローカルのリストは注釈で要素の型を書きます（コンパイル側がそれを読みます）。

```python
out: list[str] = []
for i, s in enumerate(items()):
    if s != "":
        out = out + [f"{i}: {s}"]
items.set(sorted(out))
best.set(max(scores()))
```

要素が何かを問わない操作（`in`、スライス、`+`、`[::-1]`）は、値クラスでもタプルでも、どんな要素のリストにも使えます。
比べるほうは何を比べるかが要るので、`sorted` と `min` と `max` は `key=` を、`sorted` は `reverse=` も取ります。
`key=` に渡すのは、要素を一つ取るラムダか、そういうヘルパの名前です。
並べ替えは `reverse=True` でも安定で、キーが等しい要素は入ってきた順のまま残ります。

```python
by_score = sorted(players(), key=lambda p: p.score, reverse=True)
leader = max(players(), key=lambda p: p.score)
names = [p.name for p in players()]
newest = entries()[::-1]
```

`reversed(xs)` は Python ではイテレータなので、`for` で後ろから回すのに使います。
リストがほしいところでは、Python でもリストになる `xs[::-1]` を書きます。

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

## キャンバス

キャンバスは、仮想的なピクセルを一つずつ描いていく格子です。
`width` と `height` がそのピクセル数で、`scale` は仮想の1ピクセルが論理ピクセル何個分かを表します。
`canvas(160, 120, scale=4)` は画面上で 640x480 を占めます。
描画命令はブロックの中に並べます。

```python
with canvas(160, 120, scale=4, background=0, palette=Game.palette):
    rect(Game.x, Game.y, 8, 8, 7)
    circle(30, 20, 4, 12)
    pixel_text(4, 4, f"SCORE {Game.score}", 7)
```

色はすべて数値です。
アプリが宣言した `palette`（16進の色のリスト）の何番目か、という意味の番号です。
色をパレットの番号で指すのは、ドット絵を扱う道具に共通のやり方です。
そのために書かれた描画のコードは、番号を書き換えずにそのまま移せます。
範囲を越えた番号は最後の色で描きます。
見えなくなるより、間違った色が見えるほうが直せるからです。
palette が空のキャンバスはマゼンタで描きます。

```python
@store
class Game:
    palette: list[str] = ["#000000", "#2b335f", "#7e2072", "#19959c"]
```

命令は `pixel`、`line`、`rect`、`rect_outline`、`circle`、`circle_outline`、`triangle`、`triangle_outline`、`sprite`、`pixel_text` です。
座標は整数だけです。
ピクセルの格子に半分のピクセルはないので、浮動小数点数は `int(...)` を書くように名指しで断ります。
`sprite(x, y, source, u, v, w, h)` は PNG の一部を切り出して置きます。
`colkey=` は写さない色の番号で、`flip_x=` と `flip_y=` は左右と上下の反転です。
`pixel_text` はキャンバス自身が持つ 4x6 のフォントで、ピクセルの格子の上に文字を書きます。

キャンバスの中の `for` は普通のループです。
本体が描くものが、その場所でフレームに加わります。

```python
with canvas(160, 120, scale=4, palette=Game.palette):
    for e in Game.enemies:
        sprite(e.x, e.y, "assets/sheet.png", 0, 16, 8, 8, colkey=0)
```

回せるのはビューが直接読めるリスト（`State` のセル、ストアのフィールド、モデル自身のフィールド）で、その要素はスカラーか value クラスです。
`for i, e in enumerate(...)` と書けば、要素の隣に添字も束縛できます。
`for i in range(2):` も書けます。
こちらはその場で展開されるので、範囲は書き下した数値です（64 個まで）。
ループが、それが並べるはずだった要素そのものになるためです。
これらのループはキャンバスに限らず、どのコンテナの中でも同じように書けます。

描画命令は要素ではありません。
[共通のプロパティ](tour-ui.md#共通のプロパティ)を一つも取らず、キャンバスの中のものはクリックできず、アクセシビリティの木ではキャンバス全体が1枚の画像です。
何を描いているかを伝える手段は `a11y_label=` だけです。
ダンプに出るのはフレームそのもので、1命令が1行になります。
だから `yokan gate` は、両方の実行が描こうとした絵を比べられます。

```console
Canvas(160x120, scale=4, bg=#000000)[
  Rect(56, 100, 8, 8, #eeeeee)
  PixelText(4, 4, "SCORE 1250", #eeeeee)
]
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
`.items()` は対で回ります。順序は同じ挿入順です。

リストを値に持つ辞書でグループ分けができます。

```python
groups: State[dict[str, list[str]]] = State({})

for w in words():
    groups[w[0]] = groups().get(w[0], []) + [w]
```

## タプル

タプルは、いくつかの値を一つにまとめたものです。
Python と同じ書き方で書き、同じ読み方で読みます。

```python
pair: State[tuple[str, int]] = State(("momo", 4))
rows: State[list[tuple[str, int]]] = State([])


def measure(word: str) -> tuple[str, int]:
    return (word.upper(), len(word))


def scan():
    label, n = measure("hello")          # 分解
    first = pair()[0]                    # 位置はリテラル
    whole, rest = divmod(n, 3)
    for name, count in rows():           # 行ごとに対
        total.set(total() + count)
    for key, value in prices().items():  # 辞書も対で回る
        seen.set(seen() + key)
```

部分はそれぞれ自分の型を持つので、位置はリテラルで書きます。
計算した位置だと型が一つに決まりません。
部分は二つ以上で、同じ形を state にもフィールドにもリストの要素にも引数にも返り値にも置けます。
