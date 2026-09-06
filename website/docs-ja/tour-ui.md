# コンポーネントとスタイル

[ツアー](tour.md)の続きです。
スロット付きコンポーネント、名前付きスタイル、テーマ、アニメーション、ウィンドウを見ます。

## コンポーネント

再利用したいビューの断片は **コンポーネント**（`@component`）にします。
インスタンスごとの状態は `local` で持ちます（呼び出し位置ごとに独立していて、再描画をまたいでも残ります）。

```python
@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with row(spacing=6):
        text(f"{label}: {n()}")
        button(f"+{step}", on_click=lambda: n.set(n() + step))
```

子要素を受け取るコンポーネントは、`slots=True` を付けて宣言します。
渡された子は `slot()` の位置に差し込まれます。
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
子から親へ値を返すときは、これを使います。

```python
@component
def field(label: str, cell: State[str]):
    with row(spacing=6):
        text(label)
        text_field(cell(), on_change=cell.set)

field("name", name)
field("city", city)
```

ハンドラもセルも呼び出し側が持っているので、それを受け取るコンポーネントは、呼び出し箇所ごとに別のビューになります。
同じものを渡している二か所は、一つのビューを共有します。

`local` の状態は、呼び出し位置で区別されます。
呼び出しの並びを入れ替えると、状態の対応も入れ替わります。

## 共通のプロパティ

どの要素も、次の共通のプロパティを同じ名前と同じ意味で取ります。

- **`tooltip="…"`**：ポインタを置いたときに一行を表示します。
  ポインタが乗っていなくてもダンプには出るので、検証スクリプトからも見えます。
- **`role=` / `a11y_label=`**：`role=` は、要素が自分で決めている役割（スクリーンリーダーの "button"、"heading"、"list" など）を上書きします。
  `a11y_label=` は読み上げられる名前です。
  ヘッドレススクリプトの `a11y` ステップが、その木を印字します（`demo/labels.py`）。
  `checkbox`、`switch`、`progress` は自分のラベルで名前が決まるので、`a11y_label=` は取りません。
- **`disabled=True`**：要素を薄くして無効にします。
  ウィンドウでは押せません。
  その要素を狙ったスクリプトのステップは受け付けられますが、何も起きません。
  ダンプにはその状態が出ます。
- **`width=` / `height=` / `min_width=` / `max_width=`**：大きさを与えます。
  自前の `width=` / `height=` を持つ要素（`button`、`image`、`svg`、`text`、チャート、`progress`）は、その値をそのまま使います。
- **`theme=`、`animate=` / `easing=` / `enter=` / `exit=`、`col_span=` / `row_span=`**：それぞれ[スタイルとテーマ](#スタイルとテーマ)、[アニメーション](#アニメーション)、`grid` の節で扱います（`demo/shared.py`）。

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
ビューが表示する他のものと同じで、イベントのたびに読み直されます。

テーマは `theme=` で、その要素から下にまとめて当てます。
値にはリテラルも状態の読み出しも書けるので、アプリが自分のパレットを状態として持てます。

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

いちばん外側のコンテナに当てれば、ウィンドウの背景色までテーマに従います。

## アニメーション

要素に `animate=`（ミリ秒）を付けると、その要素の変化が補間されます。
`easing=` は `"linear"`、`"in"`、`"out"`、`"inOut"` から選びます。
`enter=True` / `exit=True` を付けると、要素が現れるときと消えるときにも掛かります。

```python
text("OUTAGE — api is down", animate=140, easing="out", **pill_crit)
```

## ウィンドウ

タイトルとサイズはアプリが `run` で宣言します。

```python
run(view, title="OpsBoard", width=1100, height=820, on_start=boot)
```

`width` / `height` は論理ピクセルで、対で指定します（省略時はエンジンの既定値）。
`padding=` は、ウィンドウとアプリの木の間の余白です。
指定しなければ 16px です。
`padding=0.0` にすると、ウィンドウの端まで描けます。
キャンバスや地図のように、アプリそのものが一枚の絵であるときはこちらを使います。
この宣言はリリース版のバイナリにもそのまま引き継がれます。
`quit()` はどのハンドラからでも呼べて、ウィンドウを閉じます。
ヘッドレス実行には閉じるウィンドウがありません。
スクリプトは残りのステップを走り切り、両方の実行が同じダンプを出します。
その意味で、終了はゲートが確かめられない数少ないものの一つです。
`on_start` はマウント直後に一度だけ走るハンドラです。
失敗したときは、その内容を表示して実行を続けます（起動データの読み込みや、乱数の種まきに使います）。
起動時の処理を書く場所は `on_start` だけです。
モジュールのトップレベルに置けるのは宣言だけで、そこに書いた文（`count.set(5)` や `fs.write_text(...)`）は断られます。
コンパイル済みのアプリはモジュールを読むだけで、実行はしないためです。

