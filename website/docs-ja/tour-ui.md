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

