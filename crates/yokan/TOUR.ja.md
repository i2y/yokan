# Yokan 言語ツアー

Yokan（羊羹）は、静的に型付けされた Python のサブセットをネイティブコードにコンパイルする、デスクトップアプリのための処理系です。
このツアーはその書き方を一周します。
載っているコードはすべて、いまの Yokan でそのまま動きます。
まだできないことは、末尾の[今できないこと](#今できないこと)に理由付きでまとめてあります。

Yokan のアプリは普通の Python ファイルです。
開発中は本物の CPython で動き、配るときは同じソースがネイティブバイナリにコンパイルされます。
そして**ゲート**が、同じ操作（クリックや入力の並び）を開発版とリリース版の両方に流して結果をバイト単位で突き合わせ、二つが同じに振る舞うことをアプリごとに検証します。
このツアーで「コンパイルされる」と書いてあるものは、すべてこの検証を通っています。
以降の各節では、この検証の話は繰り返しません。

## 目次

1. [最小のアプリ](#最小のアプリ)
2. [状態の持ち方](#状態の持ち方)
3. [ビューの書き方](#ビューの書き方)
4. [フォーム部品](#フォーム部品)
5. [ハンドラと制御フロー](#ハンドラと制御フロー)
6. [算術](#算術)
7. [文字列](#文字列)
8. [リスト、チャート、仮想化リスト](#リストチャート仮想化リスト)
9. [辞書](#辞書)
10. [Value クラスとインターフェース](#value-クラスとインターフェース)
11. [メモリ管理](#メモリ管理)
12. [直和型と match](#直和型と-match)
13. [Optional と Enum](#optional-と-enum)
14. [コンポーネント](#コンポーネント)
15. [スタイルとテーマ](#スタイルとテーマ)
16. [アニメーション](#アニメーション)
17. [ウィンドウ](#ウィンドウ)
18. [エラー処理](#エラー処理)
19. [標準ライブラリ](#標準ライブラリ)
20. [Rust crate を呼ぶ](#rust-crate-を呼ぶ)
21. [CPython エスケープ](#cpython-エスケープ)
22. [重い処理とタイマーとキー](#重い処理とタイマーとキー)
23. [型チェッカーとの併用](#型チェッカーとの併用)
24. [ヘッドレス実行とゲート](#ヘッドレス実行とゲート)
25. [リリース](#リリース)
26. [本格アプリの例](#本格アプリの例)
27. [今できないこと](#今できないこと)

## 最小のアプリ

```python
# /// script
# dependencies = ["yokan"]
# ///
from yokan import State, button, column, run, text

count: State[int] = State(0)

def view():
    with column(spacing=12, padding=16):
        text(f"count: {count()}", size=34)
        button("+1", on_click=lambda: count.set(count() + 1))

if __name__ == "__main__":
    run(view, title="counter")
```

三つの動かし方があります。

```console
$ uv run app.py                                    # 開発: CPython + ライブリロード
$ yokan gate app.py --script "click:+1,click:+1"  # 検証: 開発版とリリース版の突き合わせ
$ yokan build app.py --release                    # リリース: ネイティブバイナリだけを作る
```

先頭の 3 行コメントは uv への依存宣言です。
これがあると、`uv run app.py` が必要なものを揃えてそのまま動きます。
`uv run` 中にファイルを編集して保存すると、ウィンドウは開いたまま、画面もハンドラも新しいコードに入れ替わり、状態はそのまま残ります。
`if __name__ == "__main__":` のガードは必須です（リロードの仕組み上、これが無いと二重起動を試みるため）。

## 状態の持ち方

状態を持つ道具は三つで、選び方はこうです。

- **一つの値**なら `State[T]`。
- **アプリ全体で共有するひとまとまりの状態**（カート、設定、画面ごとの状態）なら**ストア**（`@store`）。フィールドだけでもよく、操作はメソッドになります。
- **何個も作れて、変更に画面が追随してほしいオブジェクト**なら**モデル**（`@model`）。

変わらない値は状態ではありません。
モジュールレベルに書いたリテラル（`LIMIT = 10`、`NAMES = ["a", "b"]`）は宣言で、ハンドラからもビューからも名前で読めます。

### State

アプリの状態は、モジュールレベルに `State` で宣言します（`from yokan import State`）。
型注釈は必ず書きます（コンパイル時の型はこの注釈から決まります）。
読みは `count()`、書きは `count.set(v)` です。

```python
count: State[int] = State(0)
name: State[str] = State("")
show: State[bool] = State(False)
items: State[list[str]] = State([])
prices: State[dict[str, int]] = State({"apple": 120})
```

使える型は int、str、float、bool、それらのリストと辞書、Optional（`int | None`）、Enum、そして後述の Value クラスと直和型です。
int の状態は書き込みのたびに 64 bit 整数の範囲をチェックするので、数の振る舞いが開発中とリリース後で変わりません。

### ストア

**ストア**（`@store`）は、アプリにひとつだけある、フィールドとメソッドの置き場です。
クラス名がそのままストアで、`Cart.add(...)` のように呼び、`Cart.total` のように読みます。

```python
@store
class Cart:
    items: list[str] = []
    total: int = 0

    def add(self, name: str, price: int) -> None:
        self.items = self.items + [name]
        self.total += price

    def take_all(self, xs: list[str]) -> None:
        for x in xs:
            self.items = self.items + [x]

button("add", on_click=lambda: Cart.add("apple", 120))
button("clear", on_click=Cart.clear)
text(f"n={len(Cart.items)} total={Cart.total}")
```

フィールドは State と同じ型が使えます。
メソッドの中身はハンドラと同じ書き方で、ストア同士の呼び合いもできます。
メソッドの引数は int、float、str、bool、それらの `list[...]`、Value クラス、Enum が取れ、キーワード引数と既定値も Python と同じように書けます。
返り値の型を書いたメソッドは `return <式>` で終わり、ハンドラはその値を受け取れます（`Cart.count()`）。
ビューは状態を読む場所なのでメソッドは呼べません。読み取り専用の形が `@property` で、式に名前を付けたものとして、フィールドと同じ場所で使えます。

```python
    @property
    def label(self) -> str:
        return f"{len(Cart.items)} items"

    @staticmethod
    def yen(n: int) -> str:
        return f"¥{n}"
```

ストアの `@staticmethod` はクラスの中に置いた普通の関数で、純粋ヘルパと同じくビューからも呼べます。

### モデル

**モデル**（`@model`）は何個でも作れるオブジェクトで、ビューが読んだフィールドが変わると画面が追随します。
フィールドにはデフォルトが要り、メソッドはハンドラと同じ書き方です。

```python
@model
class Circle:
    r: float = 1.0
    def grow(self, by: float) -> None:
        self.r += by

left = Circle()
right = Circle()
button("grow", on_click=lambda: left.grow(0.5))
```

モデルはモデルを参照できます。
所有する参照は `Node | None`（またはモデルのリスト `list[Node]`）、所有しない逆向きの参照は `Weak[Node]` で書きます。
参照フィールドは None（リストは []）から始めて、ハンドラの中で繋ぎます。

```python
from yokan import Weak, model, store

@model
class Node:
    label: str = "n"
    kid: Node | None = None
    parent: Weak[Node] = None      # 逆向きは所有しない

@store
class Tree:
    root: Node | None = None

    def build(self) -> None:
        a = Node()
        a.label = "alpha"
        b = Node()
        b.label = "beta"
        a.kid = b
        b.parent = a               # 循環しない: parent は Weak
        self.root = a
```

親子で互いを指すとき、両方を所有にすると誰も手放せなくなります。
逆向きを `Weak` にしておくと、親を手放した瞬間（`self.root = None`）に子までまとめて解放され、残った側から `Weak` を読むと None が返ります。

読みは walrus の絞り込みで書きます（Optional の節と同じ形で、モデルの参照にもそのまま効きます）。

```python
if (r := Tree.root) is not None:
    text(f"root: {r.label}")
```

データそのものは後述の**Value クラス**で持ち、ストアのフィールドに置くのが基本形です。
モデルは「共有されて、書き換わって、画面がそれに追随する」ものにだけ使います。

## ビューの書き方

ビューは `with` でコンテナを開き、その中で要素のコンストラクタを呼ぶだけです。
開いているコンテナに自動で追加されます。
ビュー関数は状態から画面を組み立てる純粋な関数で、書き換えはハンドラの仕事です。

```python
def view():
    with column(spacing=10, padding=16):
        text(f"hello {name()}", size=20, color="accent")
        with row(spacing=6):
            text_field(name(), placeholder="name", on_change=name.set)
            button("clear", on_click=lambda: name.set(""))
```

要素カタログ：`text`、`button`、`text_field`、`checkbox`、`switch`、`slider`、`select`、`radio_group`、`tab_bar`、`column`、`row`、`grid`、`stack`、`list_view`、`scroll_view`、`h_scroll_view`、`data_table`、`modal`、`image`、`svg`、`bar_chart`、`line_chart`、`progress`、`spinner`。
`grid(columns=, rows=)` は等分のトラックを敷き、中のボタンは `col_span=` / `row_span=` でセルをまたげます（`demo/calcgrid.py` が grid 一枚のキーパッドです）。
`data_table` は表そのものを描き、中の最初の `row` がヘッダー行、以降の `row` が交互に色の付くデータ行になります。
列は、同じ列のセルに同じ `grow` を与えると揃います（`demo/table.py` では数値の列に `align="right"` を指定しています）。
サンプルは要素を裸で import します（`from yokan import button, column, run, …`）。
名前空間で呼びたい場合は `import yokan as ui`（`button`、`run`）もそのまま使え、どちらの綴りも同じにコンパイルされます。

テキストへの値の埋め込みは f-string です。
int、str、float、bool、Enum の値がそのまま描画でき、表示は Python の `str()` と同じです（`2.0` は `2.0`、`True` は `True`、`Mood.HAPPY` は `Mood.HAPPY`）。
float の小数点以下を揃えたいときは `f"{x:.1f}"` の形式指定も使えます。
`{}` の中では `+`、`-`、`*` の計算も書けます（`f"{n * 2 + 1}"`）。

条件分岐はビューの中の普通の `if` / `elif` / `else` です。
モーダルは、置けば開いていて、置かなければ閉じています。
だから `if` で包みます。

```python
if show():
    with modal():
        text("confirm?")
        button("yes", on_click=lambda: (done.set(True), show.set(False)))
```

## フォーム部品

値の入力はどれも同じ形です。
表示する値を状態から渡し、変更はハンドラが**新しい値ひとつ**を受け取って状態に書き戻します。

```python
from yokan import store

@store
class Settings:
    dark: bool = False
    wifi: bool = True
    volume: float = 5.0
    fruits: list[str] = ["apple", "banana", "cherry"]
    fruit: int = 0
    tabs: list[str] = ["General", "Details"]
    tab: int = 0

    def set_dark(self, on: bool) -> None:
        self.dark = on

    def set_wifi(self, on: bool) -> None:
        self.wifi = on

    def set_volume(self, v: float) -> None:
        self.volume = v

    def pick_fruit(self, i: int) -> None:
        self.fruit = i

    def pick_tab(self, i: int) -> None:
        self.tab = i


checkbox("Dark mode", checked=Settings.dark, on_change=Settings.set_dark)
switch("Wi-Fi", checked=Settings.wifi, on_change=Settings.set_wifi)
slider(value=Settings.volume, min=0.0, max=10.0, step=1.0, on_change=Settings.set_volume)
select(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
radio_group(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
tab_bar(labels=Settings.tabs, active=Settings.tab, on_change=Settings.pick_tab)
```

- **checkbox / switch**：ラベルと `checked=`。ハンドラは新しい bool を受け取ります。検証スクリプトでは `click:<ラベル>` がトグルです。
- **slider**：`value=` と `min=` / `max=` / `step=`。ハンドラは新しい float。スクリプトは `slide:<値>`（範囲に収め、step に吸着）。
- **select / radio_group / tab_bar**：選択肢のリストと現在位置。ハンドラは選ばれた**インデックス**。スクリプトは `select:<ラベル>`。

タブの中身の切り替えは、`tab_bar` の下に普通の `if` / `elif` を書くだけです。

## ハンドラと制御フロー

ハンドラは三つの形で渡せます。
lambda（複数の操作はタプル `lambda: (a.set(x), b.set(y))`）、モジュールレベルの def、そしてストアのメソッド参照（`on_click=Cart.clear`）です。

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
```

行数の多いリストは `list_view` に渡します。
**仮想化**されていて、行を作る関数 `row(i)` は見えている範囲についてだけ呼ばれます（10 万行でも十数回）。

```python
def row(i):
    return text(items()[i])

list_view(len(items()), row, item_height=22.0, height=200.0)
list_view(len(items()), row, item_height=22.0, grow=1.0)   # 親の残り高さを埋める
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

読みは `.get`、書きはキー単位、数えるのは `len`、回すのは `sorted()` です。
キーには str なら何でも書けます（リテラル、状態の読み、ループ変数）。

```python
prices["cherry"] = 200                 # キー単位の書き込み
picked.set(prices().get("apple", -1))  # 読み: 無いときは default
if "cherry" in prices(): ...           # 所属
len(prices())                          # 件数

def scan():
    for k in sorted(prices()):         # キー順で回る
        last.set(k)
```

素の `d[k]` 読みと素の `for k in d` は断られます。
`d[k]` は無いキーをどうするかを、`for k in d` はどの順で回るかを、書き手が決めないまま進めてしまう形だからです。
`get` と `sorted()` はその決定を式の上に出します。

## Value クラスとインターフェース

データそのものは**Value クラス**で持ちます。
`@value` を付けたクラスがネイティブの構造体にコンパイルされます（`@dataclass(frozen=True)` と同じもので、その綴りでも書けます）。
Value クラスは不変なので、書き換えは `replace` で新しい値を作る形です。
フィールドには、先に宣言した別の Value クラスも置けます（入れ子の値）。

```python
from dataclasses import replace

@value
class Point:
    x: int
    y: int = 0

sel: State[Point] = State(Point(3, 4))
sel.set(replace(sel(), x=10))
text(f"x={sel().x}")
```

Value クラスにはメソッドも書けます。
演算子の特殊メソッド（`__add__`、`__sub__`、`__mul__`）を定義すると、`+` `-` `*` がその意味になります（開発中は Python がそのまま呼び、リリース版では同じ計算がコンパイルされて呼ばれます）。
本文は `return 式` の一文です（不変の値には代入するものがないため）。

```python
@value
class V2:
    x: int
    y: int

    def __add__(self, o: "V2") -> "V2":
        return V2(self.x + o.x, self.y + o.y)

    def __mul__(self, k: int) -> "V2":
        return V2(self.x * k, self.y * k)

    def dot(self, o: "V2") -> int:
        return self.x * o.x + self.y * o.y

c.set(a() + b() * 2)      # 演算子は特殊メソッドへ
d.set(a().dot(b()))       # 普通のメソッドはハンドラから
```

インターフェースは `typing.Protocol` です。
Protocol を基底に挙げたモデルがその実装になり、Protocol 型の引数を取るヘルパはどの実装を渡しても動きます（実装ごとに特殊化してコンパイルされます）。

```python
class Shape(Protocol):
    def area(self) -> float: ...

@model
class Circle(Shape):
    r: float = 1.0
    def area(self) -> float:
        return self.r * self.r * 3.0

def area_of(s: Shape) -> float:
    return s.area()
```

## メモリ管理

手で解放するものはありません。
覚える形は二つだけです。

- **値**（Value クラス、リスト、辞書、文字列）はコピーの意味を持ちます。
  渡した先で書き換わっても、元の側は変わりません。
  リリース版は書き換わる瞬間まで実体を共有する（コピーオンライト）ので、大きなリストを渡しても複製のコストはかかりません。
- **モデル**（と、それを持つストア）は参照です。
  リリース版は参照カウントで管理し、最後の所有が外れた代入のその場で解放します。
  ヒープを走査する GC はなく、停止もありません。

この二つから、日々の作法がそのまま出ます。

- データは Value クラスとリストで持ち、ストアのフィールドに置く。モデルは「共有されて、書き換わって、画面が追随する」ものだけにする。
- ハンドラの中で作って外に渡さなかったモデルは、ハンドラを抜けた時点で解放されます。ループの中で作る一時オブジェクトも同じです。
- 所有の鎖を断てば（`self.root = None`）、その下がまとめて解放されます。生き残った側から `Weak` を読むと None が返ります。

循環だけが例外です。
互いに所有し合うオブジェクトは、鎖を断っても誰も手放せず、リリース版では解放されません（リークであって、クラッシュではありません）。
逆向きの参照を `Weak` にして、循環を作らないのが作法です。
なお開発中の CPython には循環回収があるので、循環を作ってしまったときのメモリの振る舞いだけは二つの実行で同じになりません。
ゲートが比べるのは画面で、メモリは検証の対象外だからです。

生きているオブジェクトの数は、ヘッドレス実行の `mem` ステップでいつでも数えられます。

## 直和型と match

Value クラスを `type` エイリアスで束ねると、`match` で分岐できる選択肢の型（直和型）になります。
`match` はハンドラでもビューでも使え、`case Degraded(services):` のような分解もそのまま書けます。

```python
@value
class Healthy: pass
@value
class Degraded: services: int
@value
class Outage: service: str

type Health = Healthy | Degraded | Outage

health: State[Health] = State(Healthy())

# ビューの中で:
match health():
    case Healthy():
        text("ALL SYSTEMS NOMINAL")
    case Degraded(services):
        text(f"DEGRADED — {services} service(s)")
    case Outage(service):
        text(f"OUTAGE — {service} is down")
```

case の抜けはコンパイル時に指摘されます。
バリアントのフィールドにデフォルトは書けず、一つのバリアントは一つの直和型にだけ属します。
腕にはガードと `|` の並記が書け、ガードが外れたときは Python と同じく下の腕に落ちます。

```python
match health():
    case Degraded(services) if services > 3:
        text("badly degraded")
    case Healthy() | Degraded(_):
        text("fine enough")
    case _:
        text("down")
```

## Optional と Enum

Optional は状態にもフィールドにも書けます（`last: int | None = None`）。
絞り込みは walrus の節で見たとおりです。

Enum は普通の `class Mood(Enum)` がそのままコンパイルされます。
`.name` と `.value` は Python と同じ値を返し（`auto()` は 1 から数えます）、`for m in Mood:` は宣言順にメンバーを回ります。
`match` の case は `Mood.MEMBER` か `_` で、抜けは指摘されます。
テキストに入れると Python と同じ `Mood.HAPPY` の形で描画されます。

## コンポーネント

再利用したいビューの断片は **コンポーネント**（`@component`）にします。
インスタンスごとの状態は `local` で持ちます（呼び出し位置ごとに独立で、再描画を生き延びます）。

```python
@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with row(spacing=6):
        text(f"{label}: {n()}")
        button(f"+{step}", on_click=lambda: n.set(n() + step))
```

子要素を受け取るコンポーネントは `slots=True` で宣言し、`slot()` の位置に差し込まれます。
使う側は `with` で渡します。

```python
@component(slots=True)
def card(title: str):
    with column(border_width=1.0, border_color="accent", padding=8):
        text(title, size=18)
        slot()

with card("counters"):
    counter("a", 1)
    counter("b", 10)
```

コンポーネントはコールバックや `State` のセルも受け取れます。
子から親に返す手段がこれです。

```python
@component
def field(label: str, cell: State[str]):
    with row(spacing=6):
        text(label)
        text_field(cell(), on_change=cell.set)

field("name", name)
field("city", city)
```

ハンドラもセルも呼び出し側のものなので、それを受け取るコンポーネントは呼び出し箇所ごとのビューになります（同じものを渡す二か所は一つを共有します）。

`local` は呼び出し位置で見分けられています。
呼び出しの並びを入れ替えると、状態の対応も入れ替わります。

## スタイルとテーマ

スタイルは名前を付けた辞書で、`**` で要素に展開します（一つの要素に展開できるのは一つ）。
`|` で合成できます。

```python
chip = style(size=18, color="accent")
key = style(background="surface", hover_background="surfaceHover")
hot = key | style(background="#fab387")

text(f"n={n()}", **chip)
```

色は 16 進のリテラルのほかに**テーマトークン**が書けます。
`windowBg`、`panel`、`surface`、`surfaceHover`、`border`、`text`、`textDim`、`accent` などが、その場のテーマに応じた色に解決されます。

スタイルの値はリテラルだけでなく状態からも取れます（`size=zoom()`、`color=Look.tone`、`padding=Look.pad * 2`）。
ビューは表示する他のものと同じく、イベントのたびに読み直します。

テーマは `theme=` で、その要素から下にまとめて当てます。
値はリテラルでも状態の読みでもよいので、アプリが自分のパレットを状態として持てます。

```python
mode: State[str] = State("dark")

def flip():
    if mode() == "dark":
        mode.set("light")
    else:
        mode.set("dark")

with column(background="windowBg", grow=1.0, theme=mode()):
    ...
    button("theme", on_click=flip)
```

いちばん外側のコンテナに当てれば、ウィンドウの背景色ごとテーマに従います。

## アニメーション

要素に `animate=`（ミリ秒）を付けると、その要素の変化が補間されます。
`easing=` は `"linear"`、`"in"`、`"out"`、`"inOut"` から選び、`enter=True` / `exit=True` で出入りにも掛かります。

```python
text("OUTAGE — api is down", animate=140, easing="out", **pill_crit)
```

## ウィンドウ

タイトルとサイズはアプリが `run` で宣言します。

```python
run(view, title="OpsBoard", width=1100, height=820, on_start=boot)
```

`width` / `height` は論理ピクセルで、対で指定します（省略時はエンジンの既定値）。
この宣言はリリース版のバイナリにもそのまま引き継がれます。
`on_start` はマウント直後に一度だけ走るハンドラで、失敗しても表示して続行します（起動データの読み込みや、乱数の種まきに使います）。
起動時の処理を書く場所は `on_start` だけです。
モジュールのトップレベルに置けるのは宣言だけで、そこに書いた文（`count.set(5)` や `fs.write_text(...)`）は名指しで断られます。
コンパイル済みのアプリはモジュールを読むだけで、実行はしないためです。

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

`from yokan import fs, sqlite, http, math, json, time, strings, random, clipboard, notify` で使います。
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
- **clipboard**：`set_text(s)` / `get_text()` — システムのクリップボード。ウィンドウでは他のアプリケーションとやり取りし、ヘッドレス実行では自分の中に閉じるので、コピーと貼り付けも他の操作と同じように検証できる
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

## 型チェッカーとの併用

yokan は型スタブを同梱しているので、pyright（VS Code の Pylance）によるチェックがそのまま働きます。
スタブは実行時の形をそのまま伝えます。
`@store` はクラス名がそのままインスタンスなので、`Settings.set_dark(True)` は正しくメソッド呼び出しとして数えられ、`on_change=Settings.set_dark` のようなハンドラ渡しも `(bool) -> None` として通ります。
`@model` と `@value` はフィールドどおりのコンストラクタを持つと伝わり、`Weak[Node]` はチェッカーには `Node | None` として見えます（実際、読みの意味はその通りです）。

mypy には、クラスデコレータによる型の変換を適用しないという既知の制限があります。
このため mypy では `@store` のメソッド呼び出しが「self が渡されていない」と誤検出されます。
チェックには pyright を推奨します。

型チェッカーが見るのは Python の型で、方言の境界ではありません。
そちらは `yokan check app.py` が受け持ちます。
アプリが import するモジュールをすべて翻訳器に通し、最初の拒否を `ファイル:行:列` の形で示し、方言の内側なら何も言いません。
コンパイラを起動しないので、編集しながら何度でも回せます。

## ヘッドレス実行とゲート

アプリはウィンドウなしでも動かせます。
検証はここから始まります。

```console
$ PIXIE_SCRIPT="click:+1,input:Momo" uv run app.py
```

ステップの語彙は `click[@n]:<ラベル>`、`input[@n]:<テキスト>`、`submit[@n]`、`slide[@n]:<値>`、`select[@n]:<ラベル>`、`advance:<ms>`、`theme:light|dark`、`a11y`、`mem`、`dump`。
`@n` はツリー順で n 番目の一致を選ぶので、同じラベルのボタンが並ぶ行にも届きます（`click@2:削除`）。
`dump` はその時点の画面を出力します。これで途中の状態も検査対象になり、最初と最後だけを見る形ではなくなります。
テキストに含めるカンマは `\,` と書きます（`input:hello\, world`）。
ステップの前後で画面の内容がテキストになって標準出力へ出力され、テストからは `yokan._headless(view, state, script)` が同じ文字列を返します。

**ゲート**は同じスクリプトを開発版とリリース版の両方で再生し、ダンプを突き合わせます。

```console
$ yokan gate app.py --script "click:+1,input:Momo" --release
GATE OK — 2 dump lines identical across tiers
```

ファイルや DB に書くアプリは `--fresh path/to/file.db` を付けると、先に走った側の書き込みが後の側の初回読みに漏れません。
PEP 723 の依存を持つアプリは、ゲート自体を `uv run --with <dep>` の下で回します。

## リリース

```console
$ yokan build app.py --release              # ネイティブバイナリ（検証なし）
$ yokan build app.py --release --app        # macOS の .app バンドル
$ yokan build app.py --release --bundle     # @py あり: ランタイム同梱フォルダ
$ yokan build app.py --release --bundle --app   # ランタイムごと .app に
$ yokan build app.py --release --onefile    # 1 ファイル配布
```

ネイティブビルドの前提はひとつだけです。
コンパイルが依存する Rust クレート群がリポジトリに入っているので、リポジトリを clone して、その中で（または `PIXIE_REPO` を指して）`yokan` を実行します。
手順は README の「対応環境」にまとまっています。

エスケープを使わないアプリのリリースバイナリは、それ自体が自己完結です（Python へのリンクはゼロ）。
`--bundle` は Python ランタイムと宣言済み依存を同梱したフォルダを、`--onefile` は 1 ファイル（stdlib のみ約 17 MB、numpy 込み約 21 MB。初回起動でキャッシュへ展開し、以後は約 40 ms で起動）を作ります。
ゲートは単一ファイルそのものにもスクリプトを再生できます。

`--app` は `dist/<タイトル>.app` を作ります。
Dock に名前が出て、ダブルクリックで起動でき、Finder から Applications へそのまま置けます。
`--bundle --app` なら CPython ランタイムまで `.app` の中に収まり、外に何も要りません。
アプリのファイルの隣に `<名前>.png`（または `.icns`）を置くと、アイコンとして取り込まれます。
`--onefile` は 1 ファイル形式なので `.app` とは排他です。

## 本格アプリの例

`demo/opsboard/` は三モジュール構成のダッシュボードです。
直和型のヘルスモデルをビューの `match` で分岐し、ストア二つ、ラインチャート二枚、ラベル付きバーチャート、スロット付き KPI カード、`grow` で領域いっぱいに広がる仮想化アラートフィード、重大度フィルタ、`fs` によるレポート出力、テーマ切替、種付き乱数のモックテレメトリまで、この一つに入っています。
リリースビルドは 13.7 MB（strip 後 10.6 MB）で、Python へのリンクはありません。

```console
$ uv run demo/opsboard/app.py
$ yokan build demo/opsboard/app.py --release
```

小さな例は `demo/` にひとそろいあります（counter、todo、ledger、moods、geometry、cards、styled、tryfetch、pyops など）。
辞書で状態を持つ 2 本（`run(state={...})`）を除いて、どれもゲートを通っています。
その 2 本は設計上の開発専用で、ギャラリーにもそう書いてあります。

## 今できないこと

この範囲の外にあるものは、黙って挙動を変えるのではなく、名指しで断られます。
断りの文はファイル名と行と列を挙げ、該当行を引用します。

```console
$ yokan build app.py --release
widgets.py:5:40: not in the dialect — text() does not take `weight=`
        return text(label, size=12, weight=2)
                                           ^
```

今日の時点でできないことと、その理由です。

- **辞書を挿入順で回すこと**。Python の辞書は挿入順、コンパイル後の辞書はキー順で並ぶためです。`sorted()` で回す形（キー順、両方で同じ）が用意されています。
- **素の `d[k]` 読み**。無いキーの扱いを呼び出し側が決める `.get(key, default)` が読みの形です。
- **片方の分岐でしか代入していないローカルを後で読むこと**。実行されなかったとき Python なら NameError になる形です。if / else 両方で代入すれば読めます。
- **`int ** int` の負の指数**。結果の型が実行時に変わるためで、どちらかを float にすれば書けます。
- **辞書 state（`run(state={...})`）のコンパイル**。開発中は動きますが、コンパイルできる形は型付きの `State` です。
- **Protocol 束縛のヘルパをビューから呼ぶこと**（ハンドラからは呼べます）。
- **Value クラスのメソッドをビューから呼ぶこと**（ハンドラからは呼べます。ビューはフィールドを読みます）。
- **ストアとモデルのメソッドをビューから呼ぶこと**。画面を組み立てる側は状態を読むだけで、メソッドは書き込みうるためです。読み取り専用の形は `@property` で、ビューはフィールドと同じように読めます。
- **モデルのリストをビューで直接繰り返すこと**。今日は、表示したい文字列にストア側で組み立ててから `list_view` に渡します。
- **ストアのフィールドを `Weak` にすること**。ストアは所有する側なので、所有しない参照はモデル側（逆向きのポインタ）に置きます。
- **`Vec` などネイティブ側が既に使っている型名**。名指しで断られるので、別名（`V2` など）を選びます。
- **モジュールのトップレベルに置いた文**。コンパイル済みのアプリはモジュールの宣言（import、`State`、クラス、def、`style()`、型 alias、リテラル定数、`every(...)` のタイマー、`__main__` ガード）を読むだけで実行はしないため、関数の外に書いた `count.set(5)` や `fs.write_text(...)` は名指しで断られます。起動時の処理は def に書き、`run(view, on_start=setup)` で渡します。
- **ハンドラからタイマーを開始すること**。タイマーは宣言です（モジュールレベルの `every(1.0, tick)`）。ハンドラが変えるのは、ティックが読む状態のほうです。
- **`task` の `on_error=`**。失敗の経路は誤差ユニット待ちです。標準ライブラリ呼び出しの失敗は、その呼び出しを `try` / `except` で囲んで受けます。
- コンポーネントの `local` は**呼び出し位置で識別**されます。並べ替えは状態の付け替えです。
- 同じ要素オブジェクトを**二回置くこと**。一度置いた要素は使い切りで、二か所には置けません。
- **`T | None` を返すメソッド**。ストアとモデルのメソッドはスカラー、リスト、Value クラス、Enum を返せますが、Optional の返り値はまだありません。
- **ローカルの辞書**と、注釈のないローカルのリスト（`out: list[str] = []` と書けば、コンパイル側が要素の型を読めます）。
- **よく使う分を超えた str のメソッド**（`.title()`、`.zfill()`、`.format()`、`.encode()` など）。`.upper()`、`.lower()`、`.strip()` / `.lstrip()` / `.rstrip()`、`.split()`、`.join()`、`.startswith()`、`.endswith()`、`.replace()`、`.find()`、`.count()`、`len(s)`、`s[i]`、`s[a:b]`、`in` は使えます。
- **fill、align、符号、幅、`,`、精度、`d` / `f` / `e` / `%` / `s` を超える書式指定**（`#`、`b` / `o` / `x`、`n`、`g`）。
- **辞書の `.values()` / `.items()` の反復**。Python は挿入順、コンパイル後はキー順に回るためです。`sorted(d())` を回して `d().get(k, default)` で読みます。
- **一部の制御構文**：入れ子の def（クロージャにはコンパイル後の形がありません。ヘルパはモジュールレベルに書きます）と、ビューの中の条件式（ビューでは要素を `if` で分けます）。
- **Value クラスや Enum のコンポーネント引数**、そして本体がコンテナ一つでない形（先頭の `if`、複数の要素。`column` でまとめます）。コールバックと State の引数は使えます（受け取るコンポーネントは呼び出し箇所ごとのビューになります）。
- **`tuple` と `set`**。tuple にはまだコンパイル後の形がなく、Python の set の反復順はコンパイル側で再現できないため、並べ替えずに断ります。今日は `list` がどちらも代わりになります。
- **スカラー、リスト、str キーの辞書、Value クラス、Optional 以外の `@py` の署名**（モデル、入れ子のコンテナ）。
- **`print`**。stdout はヘッドレス実行の画面ダンプが出る場所なので、`log("…")` が同じ行を両方の実行で stderr に書きます。
- **標準ライブラリでは**：文字列から時刻を読み戻すこと、ファイルの属性（サイズ、時刻）とコピーや改名、ストリーミングやバイナリのダウンロード、入れ子の json の書き出し（書き出す dict やリストの中の値は str、int、float、bool のいずれか）。
- **Rust crate の境界で、ペイロード付き enum と、双子へのメソッドは、まだ越えられない。** スカラ、String、List、Optional、str キーの辞書、構造体（入れ子・幅付きフィールド込み）、enum、Result（複合型の返りも）までは越えます。残る二つの前提: ペイロード enum は rpi-gen 自体の残件、メソッドは rpi 宣言済み struct への実装接合。構造体のフィールドに enum やリストを置く形もまだで、どれも呼ぶと理由を名指しして断られます。
- 実測値はすべて macOS/arm64 のものです。ほかのプラットフォームはまだ測っていません。

このリストは、設計が決まるたびに更新されます。
背後にある設計原則は [DESIGN.md](DESIGN.md) にまとまっています。
