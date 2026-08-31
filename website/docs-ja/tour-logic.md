# 制御フローとデータ

[ツアー](tour.md)の続きです。ハンドラに書けること、CPython と同じ意味の算術、リスト、チャート、辞書を見ます。

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

`if` / `elif` / `else`、`while`、`for`（`range()`、リストの状態、リストのフィールド、リスト型の引数）、`break` / `continue`、ローカル変数（Python と同じく再代入可）が使えます。
純粋ヘルパ（引数と返り値を注釈し、`return 式` で終わる関数）はハンドラからもビューのテキストからも呼べます。

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

## リスト、チャート、仮想化リスト

リストへの追加は「連結して置き直す」形で書きます。
リリース版では 1 要素の追記にコンパイルされるので、コピーのコストはありません。

```python
items.set(items() + [x])     # 追記
items.set([])                # クリア
len(items())                 # 件数
```

後ろからの添字はリテラルで書けます。

```python
r = names()
tail.set(r[-1])              # 最後の要素（短すぎればその文が中断）
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

## 辞書

読みは `.get`、書きはキー単位、数えるのは `len`、回すのは `sorted()` です。

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

