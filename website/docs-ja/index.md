---
hide:
  - navigation
  - toc
---

<div class="yk-hero" markdown>
<img class="yk-hero__mark" src="images/logo.svg" alt="">

# Yokan

<p class="yk-hero__tag">Write Python. Ship native.</p>

<!-- 日本語は行の折り返しが空白として描画されるので、リード文は一行で書く -->
<p class="yk-hero__lede">Yokan（羊羹）は、静的に型付けされた Python のサブセットをネイティブコードにコンパイルする処理系です。書けるのは Python の一部ですが、その範囲のコードは Python とまったく同じに動きます。開発中はアプリ全体が本物の CPython で動き、リリースするときに同じソースが機械語の実行ファイルになります。<strong>その二つが同じに動くかどうかは、<code>yokan gate</code> で確かめられます</strong>。</p>

<div class="yk-hero__cta" markdown>
[はじめる](installation.md){ .md-button .md-button--primary }
[言語ツアー](tour.md){ .md-button }
[デモ](demos.md){ .md-button }
[GitHub](https://github.com/i2y/yokan){ .md-button }
</div>
</div>

<div class="yk-facts" markdown>
<div><b>13.4 MB</b><span>counter を配布した実行ファイル。Python は入らず、起動はミリ秒</span></div>
<div><b>約 1 ms</b><span>保存すると、動いているウィンドウが状態を保ったまま更新される</span></div>
<div><b>バイト単位で一致</b><span>同じ操作を両方の実行に流し、出てきた画面を突き合わせる</span></div>
</div>

![Yokan で書いたダッシュボードのデモ OpsBoard](images/opsboard.png)

*[`demo/opsboard`](https://github.com/i2y/yokan/tree/main/crates/yokan/demo/opsboard)
— 3 モジュール構成のダッシュボード。ストア 2 つ、直和型のヘルスモデルをビューの match で分岐、チャート、仮想化アラートフィード、テーマ切替。すべて Python で書かれ、1 つのネイティブバイナリになります。*

いちばん小さいアプリの全文がこちらです。

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

`uv run app.py` で動かすと、この画面が開きます（描画エンジンは Zed エディタを支える **gpui**）。

![counter を実行したところ](images/counter.png)

実行したままソースを直して保存すると、状態はそのままに、画面もハンドラの挙動も新しいコードに入れ替わります。
下の GIF がその瞬間です（別の小さなデモを編集しています）。編集の前後でティックが止まっていないことに注目してください。

![実行中のアプリのソースを編集すると、ティックを刻んだまま画面がその場で更新される](images/reload.gif)

リリースは:

```console
$ yokan build app.py --release
```

`@py` を使っていないアプリなら、できあがる実行ファイルに CPython は入りません。
Python へのリンクもゼロで、13.4 MB（strip 後 10.4 MB）、起動は数ミリ秒です。
受け取る側のマシンには何のインストールも要りません。

---

## 全体像

アプリはひとつの Python ファイルで、実行の道が二つあります。
どちらの道も同じ Rust 製の土台の上で動きます。
土台の名前が **pixie**（Yokan が経由する基盤言語）で、リリースの途中に挟まる `.pix` はその読めるソースです。
生成された `.pix` を開けば、自分のアプリが何にコンパイルされたのかを目で確かめられます。

![Yokan の全体像: ひとつのソース、開発は VM の速いループ、リリースは VM なしのネイティブバイナリ（@py があるときだけ CPython を同梱）、共有の土台、そしてゲート](images/architecture-ja.svg)

---

## 「手元では動いたのに」を仕組みで消す

同じ操作の並びを渡すと、CPython 版と機械語版の両方でそれを再生して、画面の結果をバイト単位で突き合わせます。
Yokan ではこれを**ゲート**と呼んでいます。

```console
$ yokan gate app.py --script "click:+1,input:Momo"
GATE OK — 2 dump lines identical in both runs
```

Yokan 自身のモジュール（ファイル、SQLite、HTTP、クリップボード）は、両方の実行が同じ実装を呼ぶので食い違いようがありません。
Python 自身のモジュール（`math`、`re`、`datetime` など）は、開発中は CPython が、コンパイル後は双子が答えます。その二つをつなぐのがゲートです。
まだできないことは、ツアー末尾の[今できないこと](tour-ship.md#今できないこと)に理由付きでまとまっています。

---

## なぜ Yokan？

<div class="grid cards" markdown>

-   :material-check-decagram: __配ったものが、試したものと同じ__

    クリックやキー入力を開発中の実行とビルド後のバイナリの両方に流し、出てきた画面をバイト単位で比べます。CI に置けば、違いが出た時点でビルドが落ちます。

-   :material-fire: __状態を保つホットリロード__

    `uv run app.py` して、編集して、保存。ウィンドウはそのまま、状態も生きたまま、アプリが約 1 ms で新しいコードに入れ替わります。Flutter と Dart で知られた形を、Python で。

-   :material-package-variant-closed: __渡すのはファイル 1 つ__

    `--release` は Python を含まないネイティブ実行ファイルを、`--onefile` は `@py` アプリ用に CPython ごと 1 ファイルを作ります。どちらでも受け取る側のインストールはゼロです。

-   :material-language-python: __残りの Python もそのまま__

    関数に `@py` を付ければ、実行ファイルに同梱された本物の CPython で動きます。numpy も pandas も、手持ちのコードもそのままです。

-   :material-language-rust: __Rust crate（crates.io も手元のも）__

    `yokan add app.py deunicode 1` — crates.io の version 指定でも手元の path でも、宣言すれば Yokan のコードから呼べます。crate 側は普通の Rust のままで、Yokan のために書き足すものはありません。

-   :material-shield-check: __型チェックが通る__

    同梱の型スタブで pyright / Pylance のチェックがそのまま通ります。`@store` のシングルトンも `@model` / `@value` のコンストラクタも `Weak[Node]` も、実行時の形どおりに見えます。

</div>

---

## 次はどこへ

<div class="grid cards" markdown>

-   :material-rocket-launch: __[インストール](installation.md)__

    開発は `uv run` だけ。ネイティブビルドはリポジトリを clone。いまのところ Apple silicon の macOS です。

-   :material-book-open-variant: __[言語ツアー](tour.md)__

    状態、ビュー、フォーム、メモリ、Rust crate、ゲートまで書き方を一周。末尾に「今できないこと」。

-   :material-view-gallery: __[デモ](demos.md)__

    付属デモを全部スクリーンショット付きで。最小の counter から OpsBoard まで。

-   :material-github: __[ソースコード](https://github.com/i2y/yokan)__

    コンパイラ、エンジン、デモ。

</div>

---

_名前は和菓子の羊羹からとりました。中身のぎっしり詰まったひと棹を、切り分けて配るお菓子です。ひとつの実行ファイルに詰めて配る、このアプリの姿と重なります。_

_"Python" は Python Software Foundation の商標です。_
