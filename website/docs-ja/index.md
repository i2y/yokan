---
hide:
  - navigation
  - toc
---

# Yokan

## Python で書く。ネイティブで配る。

Yokan（羊羹）は、静的に型付けされた Python のサブセットをネイティブコードにコンパイルする**処理系**です。
サブセットといっても、Python に似せた別の言語ではありません。
書けるのは Python の一部ですが、その範囲のコードは Python とまったく同じに動きます。
開発中はアプリ全体が本物の CPython で動き、リリースするときに同じソースが機械語の実行ファイルになります。
そして、**この二つが同じに振る舞うことを、ビルドのたびに自動で検証します**。

[インストール](installation.md){ .md-button .md-button--primary }
[言語ツアー](tour.md){ .md-button }
[デモ](demos.md){ .md-button }
[GitHub](https://github.com/i2y/yokan){ .md-button }

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

ビルドのたびに、同じ操作の並びを CPython 版と機械語版の両方で再生して、画面の結果をバイト単位で突き合わせます。
Yokan ではこれを**ゲート**と呼んでいます。

```console
$ yokan gate app.py --script "click:+1,input:Momo"
GATE OK — 2 dump lines identical across tiers
```

ファイルや SQLite、HTTP などの標準ライブラリが開発中も配布後も同じ実装なのは、この検証を成り立たせるためです。
まだできないことは、ツアー末尾の[今できないこと](tour-ship.md#今できないこと)に理由付きでまとまっています。

---

## なぜ Yokan？

<div class="grid cards" markdown>

-   :material-check-decagram: __ビルドごとの検証__

    「Python がコンパイルできます」ではなく、「*この*アプリが変換でき、バイナリが CPython 版とバイト単位で同じに振る舞うことをゲートが証明した」と言います。

-   :material-fire: __状態を保つホットリロード__

    `uv run app.py` して、編集して、保存。ウィンドウはそのまま、状態も生きたまま、アプリが約 1 ms で新しいコードに入れ替わります。Flutter と Dart で知られた形を、Python で。

-   :material-package-variant-closed: __渡すのはファイル 1 つ__

    `--release` は Python を含まないネイティブ実行ファイルを、`--onefile` は `@py` アプリ用に CPython ごと 1 ファイルを作ります。どちらでも受け取る側のインストールはゼロです。

-   :material-language-python: __残りの Python もそのまま__

    関数に `@py` を付ければ、実行ファイルに同梱された本物の CPython で動きます。numpy も pandas も、手持ちのコードもそのままです。

-   :material-language-rust: __Rust crate（crates.io も手元のも）__

    `yokan add app.py deunicode 1` — crates.io の version 指定でも手元の path でも、宣言すれば Yokan のコードから呼べます。開発用の pyo3 の入口もリリース用のバインディングも、crate の rustdoc JSON から自動生成されます。

-   :material-shield-check: __型チェックが通る__

    同梱の型スタブで pyright / Pylance のチェックがそのまま通ります。`@store` のシングルトンも `@model` / `@value` のコンストラクタも `Weak[Node]` も、実行時の形どおりに見えます。

</div>

---

## 次はどこへ

<div class="grid cards" markdown>

-   :material-rocket-launch: __[インストール](installation.md)__

    開発は `uv run` だけ。ネイティブビルドはリポジトリを clone。今日は Apple silicon の macOS、Linux はまもなく。

-   :material-book-open-variant: __[言語ツアー](tour.md)__

    状態、ビュー、フォーム、メモリ、Rust crate、ゲートまで書き方を一周。末尾に「今できないこと」。

-   :material-view-gallery: __[デモ](demos.md)__

    付属デモ 41 本を全部スクリーンショット付きで。最小の counter から OpsBoard まで。

-   :material-github: __[ソースコード](https://github.com/i2y/yokan)__

    コンパイラ、エンジン、デモ。

</div>

---

_名前は和菓子の羊羹からとりました。中身のぎっしり詰まったひと棹を、切り分けて配るお菓子です。ひとつの実行ファイルに詰めて配る、このアプリの姿と重なります。_

_"Python" は Python Software Foundation の商標です。_
