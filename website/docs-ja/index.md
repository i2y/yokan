---
title: "Write Python. Ship native."
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

## 全体像

同じソースが、二つの道で動きます。
どちらの道も同じ Rust 製の土台の上です。
土台の名前が **pixie**（Yokan が経由する基盤言語）で、リリースの途中に挟まる `.pix` はその読めるソースです。
生成された `.pix` を開けば、自分のアプリが何にコンパイルされたのかを目で確かめられます（`yokan translate app.py` がそれを出力します）。

![Yokan の全体像: ひとつのソース、開発は VM の速いループ、リリースは VM なしのネイティブバイナリ（@py があるときだけ CPython を同梱）、共有の土台、そしてゲート](images/architecture-ja.svg#only-dark)

![Yokan の全体像: ひとつのソース、開発は VM の速いループ、リリースは VM なしのネイティブバイナリ（@py があるときだけ CPython を同梱）、共有の土台、そしてゲート](images/architecture-ja-light.svg#only-light)

---

## どんな見た目になるか

![Yokan で書いたダッシュボードのデモ OpsBoard](images/opsboard.png)

*[`demo/opsboard`](https://github.com/i2y/yokan/tree/main/crates/yokan/demo/opsboard)
— 3 モジュール構成のダッシュボード。ストア 2 つ、直和型のヘルスモデルをビューの match で分岐、チャート、仮想化アラートフィード、テーマ切替。すべて Python で書かれ、1 つのネイティブバイナリになります。*

---

## 書いて、動かして、配る

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

配るときはこうします。

```console
$ yokan build app.py --release
```

`@py` を使っていないアプリなら、できあがる実行ファイルに CPython は入りません。
Python へのリンクもゼロで、14.7 MB（strip 後 11.3 MB）、起動は数ミリ秒です。
受け取る側のマシンには何のインストールも要りません。

---

## 「手元では動いたのに」

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


## エージェントに書かせるなら

エージェントはファイルを書き、返ってきたものを読みます。
だから、返ってくるものが何かで往復の質が決まります。
最初の二つのコマンドは、コンパイラもウィンドウもなしに約1秒で答えます。
代わりに何を書くかを名指しする拒否と、テキストになった画面です。
ゲートは最後の証明です。

![エージェントが回す往復。中心の app.py を書き、yokan check と yokan show をそれぞれ約1秒で周り、コンパイルする yokan gate で輪を離れて、出荷へ向かう](images/cycle-ja.svg#only-dark)

![エージェントが回す往復。中心の app.py を書き、yokan check と yokan show をそれぞれ約1秒で周り、コンパイルする yokan gate で輪を離れて、出荷へ向かう](images/cycle-ja-light.svg#only-light)

往復の全体は[エージェントと作る](agents.md)にあります。
エージェントに渡すガイドは [`skills/yokan/SKILL.md`](https://github.com/i2y/yokan/blob/main/skills/yokan/SKILL.md) です。

---


## ほかに入っているもの

<div class="grid cards" markdown>

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
