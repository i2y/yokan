# Yokan 言語ツアー

Yokan（羊羹）は、静的に型付けされた Python のサブセットをネイティブコードにコンパイルする、デスクトップアプリのための処理系です。
このツアーはその書き方を一周します。
載っているコードはすべて、いまの Yokan でそのまま動きます。
まだできないことは、最後のページの[今できないこと](tour-ship.md#今できないこと)に理由付きでまとめてあります。

Yokan のアプリは普通の Python ファイルです。
開発中は本物の CPython で動き、配るときは同じソースがネイティブバイナリにコンパイルされます。
そして**ゲート**が、同じ操作（クリックや入力の並び）を開発版とリリース版の両方に流して結果をバイト単位で突き合わせ、二つが同じに振る舞うことをアプリごとに検証します。
このツアーで「コンパイルされる」と書いてあるものは、すべてこの検証を通っています。
以降の各節では、この検証の話は繰り返しません。

## 最小のアプリ

```python
# /// script
# dependencies = ["yokan"]
# ///
import yokan as ui
from yokan import State

count: State[int] = State(0)

def view():
    with ui.column(spacing=12, padding=16):
        ui.text(f"count: {count()}", size=34)
        ui.button("+1", on_click=lambda: count.set(count() + 1))

if __name__ == "__main__":
    ui.run(view, title="counter")
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

ui.button("add", on_click=lambda: Cart.add("apple", 120))
ui.button("clear", on_click=Cart.clear)
ui.text(f"n={len(Cart.items)} total={Cart.total}")
```

フィールドは State と同じ型が使えます。
メソッドの中身はハンドラと同じ書き方で、ストア同士の呼び合いもできます。
メソッドの引数は int、float、str、bool、それらの `list[...]`、Value クラス、Enum が取れます。

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
ui.button("grow", on_click=lambda: left.grow(0.5))
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
    ui.text(f"root: {r.label}")
```

データそのものは後述の**Value クラス**で持ち、ストアのフィールドに置くのが基本形です。
モデルは「共有されて、書き換わって、画面がそれに追随する」ものにだけ使います。

## ビューの書き方

ビューは `with` でコンテナを開き、その中で要素のコンストラクタを呼ぶだけです。
開いているコンテナに自動で追加されます。
ビュー関数は状態から画面を組み立てる純粋な関数で、書き換えはハンドラの仕事です。

```python
def view():
    with ui.column(spacing=10, padding=16):
        ui.text(f"hello {name()}", size=20, color="accent")
        with ui.row(spacing=6):
            ui.text_field(name(), placeholder="name", on_change=name.set)
            ui.button("clear", on_click=lambda: name.set(""))
```

要素カタログ：`text`、`button`、`text_field`、`checkbox`、`switch`、`slider`、`select`、`radio_group`、`tab_bar`、`column`、`row`、`grid`、`stack`、`list_view`、`scroll_view`、`h_scroll_view`、`data_table`、`modal`、`image`、`svg`、`bar_chart`、`line_chart`、`progress`、`spinner`。
`grid(columns=, rows=)` は等分のトラックを敷き、中のボタンは `col_span=` / `row_span=` でセルをまたげます（`demo/calcgrid.py` が grid 一枚のキーパッドです）。
ここで `ui.` 経由で呼んでいるものは、`from yokan import button, column, run, …` の裸 import でも書けます。どちらの綴りも同じにコンパイルされます。
このドキュメントが `ui.` を使うのは、要素を出す行が一目で分かるようにするためです。

テキストへの値の埋め込みは f-string です。
int、str、float、bool、Enum の値がそのまま描画でき、表示は Python の `str()` と同じです（`2.0` は `2.0`、`True` は `True`、`Mood.HAPPY` は `Mood.HAPPY`）。
float の小数点以下を揃えたいときは `f"{x:.1f}"` の形式指定も使えます。
`{}` の中では `+`、`-`、`*` の計算も書けます（`f"{n * 2 + 1}"`）。

条件分岐はビューの中の普通の `if` / `elif` / `else` です。
モーダルは、置けば開いていて、置かなければ閉じています。
だから `if` で包みます。

```python
if show():
    with ui.modal():
        ui.text("confirm?")
        ui.button("yes", on_click=lambda: (done.set(True), show.set(False)))
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


ui.checkbox("Dark mode", checked=Settings.dark, on_change=Settings.set_dark)
ui.switch("Wi-Fi", checked=Settings.wifi, on_change=Settings.set_wifi)
ui.slider(value=Settings.volume, min=0.0, max=10.0, step=1.0, on_change=Settings.set_volume)
ui.select(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
ui.radio_group(options=Settings.fruits, selected=Settings.fruit, on_change=Settings.pick_fruit)
ui.tab_bar(labels=Settings.tabs, active=Settings.tab, on_change=Settings.pick_tab)
```

- **checkbox / switch**：ラベルと `checked=`。ハンドラは新しい bool を受け取ります。検証スクリプトでは `click:<ラベル>` がトグルです。
- **slider**：`value=` と `min=` / `max=` / `step=`。ハンドラは新しい float。スクリプトは `slide:<値>`（範囲に収め、step に吸着）。
- **select / radio_group / tab_bar**：選択肢のリストと現在位置。ハンドラは選ばれた**インデックス**。スクリプトは `select:<ラベル>`。

タブの中身の切り替えは、`tab_bar` の下に普通の `if` / `elif` を書くだけです。

