# コンポーネントとスタイル

[ツアー](tour.md)の続きです。スロット付きコンポーネント、名前付きスタイル、テーマ、アニメーション、ウィンドウを見ます。

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

## 共通のプロパティ

どの要素も、次の共通のプロパティを同じ名前と同じ意味で取ります。

- **`tooltip="…"`**：ポインタを置いたときに一行を表示します。置かなくてもダンプには出るので、検証スクリプトからも見えます。
- **`role=` / `a11y_label=`**：`role=` は要素が自分で導く役割（スクリーンリーダーの "button"、"heading"、"list" など）を上書きし、`a11y_label=` は読み上げられる名前です。ヘッドレススクリプトの `a11y` ステップがその木を印字します（`demo/labels.py`）。`checkbox`、`switch`、`progress` は自分のラベルで名前が決まるので `a11y_label=` は取りません。
- **`disabled=True`**：要素を薄くして無効にします。ウィンドウでは押せず、それを狙ったスクリプトのステップは受け付けられて何もせず、ダンプにその状態が出ます。
- **`width=` / `height=` / `min_width=` / `max_width=`**：大きさを与えます。自前の `width=` / `height=` を持つ要素（`button`、`image`、`svg`、`text`、チャート、`progress`）はそれをそのまま使います。
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

